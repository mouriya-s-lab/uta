use std::{
    collections::BTreeSet,
    io,
    path::PathBuf,
    process::ExitStatus,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use serde_json::{Value, json};
use tokio::{
    sync::mpsc,
    task::{JoinHandle, JoinSet},
    time::timeout,
};
use uta_base::{IntegrationId, ProcessRole};
use uta_channel::{ChildProcess, ChildSpec, Credential, SessionChannel, Spawned};
use uta_proc::{ExitConfirmed, OsProcessId};
use uta_rpc::{CallError, FrameConfig, InboundMessage, PeerEnd, PeerHandle, RpcError};
use uta_store::{Store, UnixMillis};

const TEST_TIMEOUT: Duration = Duration::from_secs(15);
static NEXT_CREDENTIAL: AtomicU64 = AtomicU64::new(0);

struct FixtureProcess {
    channel: SessionChannel,
    child: ChildGuard,
    identity: OsProcessId,
}

struct ChildGuard {
    process: Option<ChildProcess>,
    identity: Option<OsProcessId>,
}

impl ChildGuard {
    fn pid(&self) -> u32 {
        self.process
            .as_ref()
            .expect("child process is still owned")
            .pid()
    }

    fn set_identity(&mut self, identity: OsProcessId) {
        self.identity = Some(identity);
    }

    fn terminate(&self) -> io::Result<()> {
        self.process
            .as_ref()
            .expect("child process is still owned")
            .terminate()
    }

    async fn wait(mut self) -> io::Result<ExitStatus> {
        let process = self
            .process
            .take()
            .expect("child process is waited only once");
        let status = process.wait().await?;
        self.identity = None;
        Ok(status)
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(process) = &self.process {
            let _ = process.terminate();
        } else if let Some(identity) = self.identity {
            let _ = uta_proc::terminate(identity);
        }
    }
}

struct CorePeer {
    handle: PeerHandle,
    peer_task: JoinHandle<PeerEnd>,
    inbound_task: JoinHandle<()>,
    hang_started: mpsc::UnboundedReceiver<u64>,
}

impl CorePeer {
    fn start(channel: SessionChannel) -> Self {
        let (handle, mut inbound, peer_task) = uta_rpc::start(channel, FrameConfig::default());
        let peer_task = tokio::spawn(peer_task);
        let (hang_started_tx, hang_started) = mpsc::unbounded_channel();
        let inbound_task = tokio::spawn(async move {
            while let Some(message) = inbound.recv().await {
                match message {
                    InboundMessage::Request(request) => {
                        let (method, params, responder) = request.into_parts();
                        if method == "ping" {
                            let params: Value =
                                serde_json::from_slice(&params).expect("RPC params are valid JSON");
                            let result = json!({"pong": params});
                            responder.respond(Ok(&result));
                        } else {
                            responder.respond::<Value>(Err(RpcError {
                                code: -32601,
                                message: "method not found".to_owned(),
                                data: None,
                            }));
                        }
                    }
                    InboundMessage::Notification(notification) => {
                        let (method, params) = notification.into_parts();
                        if method == "hang_started" {
                            let params: Value = serde_json::from_slice(&params)
                                .expect("hang notification params are valid JSON");
                            let request_id = params
                                .get("id")
                                .and_then(Value::as_u64)
                                .expect("hang notification has a request id");
                            let _ = hang_started_tx.send(request_id);
                        }
                    }
                }
            }
        });

        Self {
            handle,
            peer_task,
            inbound_task,
            hang_started,
        }
    }

    async fn wait_for_hang_started(&mut self, expected_ids: &[u64]) {
        let mut received = BTreeSet::new();
        while received.len() < expected_ids.len() {
            let request_id = timeout(TEST_TIMEOUT, self.hang_started.recv())
                .await
                .expect("fixture did not acknowledge all hang calls")
                .expect("core inbound loop ended before hang acknowledgements");
            assert!(
                expected_ids.contains(&request_id),
                "fixture acknowledged an unknown hang call"
            );
            assert!(
                received.insert(request_id),
                "fixture acknowledged the same hang call twice"
            );
        }
    }

    async fn close(self) -> PeerEnd {
        self.handle.close();
        self.wait_end().await
    }

    async fn wait_end(self) -> PeerEnd {
        let Self {
            handle,
            peer_task,
            inbound_task,
            hang_started,
        } = self;
        timeout(TEST_TIMEOUT, async move {
            let peer_end = peer_task.await.expect("core peer task did not panic");
            inbound_task.await.expect("core inbound task did not panic");
            drop((handle, hang_started));
            peer_end
        })
        .await
        .expect("core peer did not end")
    }
}

fn unique_credential(test_name: &str) -> Vec<u8> {
    let sequence = NEXT_CREDENTIAL.fetch_add(1, Ordering::Relaxed);
    format!("uta-testkit-{test_name}-{}-{sequence}", std::process::id()).into_bytes()
}

fn spawn_fixture(credential: Vec<u8>) -> FixtureProcess {
    let spec = ChildSpec {
        program: PathBuf::from(env!("CARGO_BIN_EXE_uta-fixture-integration")),
        args: Vec::new(),
        current_dir: None,
    };
    let Spawned { channel, process } = uta_channel::spawn(&spec, Credential::from(credential))
        .expect("fixture process should spawn");
    let mut child = ChildGuard {
        process: Some(process),
        identity: None,
    };
    let identity = uta_proc::observe(child.pid()).expect("fixture identity should be observable");
    child.set_identity(identity);
    FixtureProcess {
        channel,
        child,
        identity,
    }
}

async fn call_json(handle: &PeerHandle, method: &str, params: &Value) -> Value {
    let response = timeout(TEST_TIMEOUT, handle.call(method, params))
        .await
        .expect("fixture RPC call timed out")
        .expect("fixture RPC call failed");
    serde_json::from_slice(&response).expect("fixture returned valid JSON")
}

async fn wait_child(child: ChildGuard) -> ExitStatus {
    timeout(TEST_TIMEOUT, child.wait())
        .await
        .expect("fixture process did not exit")
        .expect("fixture process wait failed")
}

fn contains_bytes(value: &str, bytes: &[u8]) -> bool {
    value
        .as_bytes()
        .windows(bytes.len())
        .any(|window| window == bytes)
}

#[tokio::test]
async fn credential_and_bidirectional_rpc_use_the_inherited_session() {
    let credential = unique_credential("rpc");
    let fixture = spawn_fixture(credential.clone());
    let core = CorePeer::start(fixture.channel);

    let returned_credential = call_json(&core.handle, "credential", &Value::Null).await;
    let returned_credential: Vec<u8> = serde_json::from_value(returned_credential)
        .expect("credential result should be a JSON byte array");
    assert!(
        returned_credential == credential,
        "fixture credential did not match the core-provided bytes"
    );

    let echo_params = json!({"nested": [1, "two", false], "marker": "round-trip"});
    let echoed = call_json(&core.handle, "echo", &echo_params).await;
    assert_eq!(echoed, echo_params);

    let callback_params = json!({"from": "core", "sequence": 29});
    let callback_result = call_json(&core.handle, "call_back", &callback_params).await;
    assert_eq!(callback_result, json!({"pong": callback_params}));

    let environment = call_json(&core.handle, "environment", &Value::Null).await;
    let args = environment
        .get("args")
        .and_then(Value::as_array)
        .expect("fixture environment includes argv");
    let env = environment
        .get("env")
        .and_then(Value::as_array)
        .expect("fixture environment includes environment values");
    assert!(
        !args
            .iter()
            .filter_map(Value::as_str)
            .any(|argument| contains_bytes(argument, &credential)),
        "credential bytes appeared in fixture argv"
    );
    assert!(
        !env.iter().any(|entry| {
            entry
                .as_array()
                .and_then(|pair| pair.get(1))
                .and_then(Value::as_str)
                .is_some_and(|value| contains_bytes(value, &credential))
        }),
        "credential bytes appeared in fixture environment values"
    );

    let peer_end = core.close().await;
    assert!(matches!(peer_end, PeerEnd::LocalClosed));
    let status = wait_child(fixture.child).await;
    assert!(
        status.success(),
        "fixture did not exit successfully after channel close"
    );
}

#[tokio::test]
async fn terminating_child_closes_pending_calls_and_clears_its_process_row() {
    const HANG_IDS: [u64; 3] = [17, 23, 43];
    let credential = unique_credential("lifecycle");
    let fixture = spawn_fixture(credential);
    let identity = fixture.identity;

    let tempdir = tempfile::tempdir().expect("temporary store directory should be created");
    let mut store = Store::open(&tempdir.path().join("uta.db")).expect("store should open");
    let previous = store
        .transact(|tx| tx.instances().begin(UnixMillis::now()))
        .expect("first instance should begin")
        .into_inner();
    let role = ProcessRole::Integration(
        IntegrationId::parse("fixture-integration").expect("fixture role id should parse"),
    );
    store
        .transact(|tx| tx.processes().register(&previous, identity, &role))
        .expect("fixture process row should register")
        .into_inner();

    let mut core = CorePeer::start(fixture.channel);
    let mut pending_calls = JoinSet::new();
    for request_id in HANG_IDS {
        let handle = core.handle.clone();
        pending_calls.spawn(async move { handle.call("hang", &json!({"id": request_id})).await });
    }

    core.wait_for_hang_started(&HANG_IDS).await;
    assert!(
        pending_calls.try_join_next().is_none(),
        "hang call completed before the child was terminated"
    );

    fixture
        .child
        .terminate()
        .expect("fixture should terminate for the lifecycle test");
    for _ in 0..HANG_IDS.len() {
        let result = timeout(TEST_TIMEOUT, pending_calls.join_next())
            .await
            .expect("pending RPC call did not complete after child termination")
            .expect("every pending call should complete once")
            .expect("pending RPC task should not panic");
        assert!(
            matches!(result, Err(CallError::Closed)),
            "pending call did not complete with CallError::Closed"
        );
    }
    assert!(pending_calls.join_next().await.is_none());

    let peer_end = core.wait_end().await;
    assert!(matches!(peer_end, PeerEnd::RemoteClosed));
    let status = wait_child(fixture.child).await;
    assert!(
        !status.success(),
        "terminated fixture unexpectedly exited successfully"
    );
    let exited: ExitConfirmed = timeout(TEST_TIMEOUT, uta_proc::confirm_exit(identity))
        .await
        .expect("OS exit confirmation timed out");
    assert_eq!(exited.id(), identity);

    drop(previous);
    let later = store
        .transact(|tx| tx.instances().begin(UnixMillis::now()))
        .expect("later instance should begin")
        .into_inner();
    let orphan_rows = store
        .processes_of_other_instances(&later)
        .expect("process rows should be readable");
    assert_eq!(orphan_rows.len(), 1);
    assert_eq!(orphan_rows[0].process, identity);
    assert_eq!(orphan_rows[0].role, role);

    store
        .transact(|tx| tx.processes().clear(exited))
        .expect("confirmed process row should clear")
        .into_inner();
    assert!(
        store
            .processes_of_other_instances(&later)
            .expect("cleared process rows should be readable")
            .is_empty(),
        "later instance still saw a process row after OS-confirmed clearing"
    );
}

#[tokio::test]
async fn core_peer_close_makes_fixture_exit_successfully() {
    let fixture = spawn_fixture(unique_credential("core-close"));
    let core = CorePeer::start(fixture.channel);

    let params = json!({"ready": true});
    let echoed = call_json(&core.handle, "echo", &params).await;
    assert_eq!(echoed, params);

    let peer_end = core.close().await;
    assert!(matches!(peer_end, PeerEnd::LocalClosed));
    let status = wait_child(fixture.child).await;
    assert!(
        status.success(),
        "fixture did not exit successfully after core close"
    );
}
