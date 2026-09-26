//! The core thread: the only owner of the SQLite connection and of the
//! instance's durable state.
//!
//! Other parts of the process reach it through [`CoreHandle`] by sending
//! closed, typed [`Command`]s over a bounded channel; each command runs to
//! completion on the core thread (one command, one transaction) and replies
//! over a oneshot. No closure, connection or transaction crosses the thread
//! boundary.
//!
//! Lifecycle, as handshakes:
//! - start: the thread opens the store and commits the fence transaction
//!   (instance row) before [`start`] returns a handle; on failure no handle
//!   exists and the thread has ended.
//! - stop: [`CoreHandle::stop`] consumes the handle; the thread writes the
//!   instance end anchor, closes the connection on itself, replies, and is
//!   joined before `stop` returns. Only then may the caller release the lock.

use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::thread::{self, JoinHandle};

use tokio::sync::{mpsc, oneshot};
use uta_proc::ExitConfirmed;
use uta_store::{
    Committed, CurrentInstance, InstanceId, InstanceRecord, OpenError, ProcessRecord, ReadError,
    SqliteError, Store, TxError, UnixMillis,
};

/// Bound on queued commands; senders wait when the core thread is behind.
const COMMAND_QUEUE: usize = 64;

/// What the core knows once the fence is taken.
#[derive(Debug)]
pub struct Ready {
    pub instance: InstanceId,
    pub previous: Option<InstanceRecord>,
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("cannot open the store: {0}")]
    Open(#[from] OpenError),
    #[error("the fence transaction did not commit: {0}")]
    Fence(TxError<SqliteError>),
    #[error("cannot read the previous instance: {0}")]
    Read(#[from] ReadError),
    #[error("cannot start the core thread: {0}")]
    Spawn(std::io::Error),
    #[error("the core thread ended before reporting readiness")]
    Vanished,
}

#[derive(Debug, thiserror::Error)]
pub enum StopError {
    #[error("the end anchor did not commit: {0}")]
    EndAnchor(TxError<SqliteError>),
    #[error("closing the store failed: {0}")]
    Close(SqliteError),
    #[error("the core thread ended without completing the stop")]
    Vanished,
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error(transparent)]
    Write(#[from] TxError<SqliteError>),
    /// The core thread is gone; the command did not run.
    #[error("the core thread is not running")]
    CoreGone,
}

/// Commands the core thread executes. Each variant carries owned values only.
#[derive(Debug)]
enum Command {
    OtherInstanceProcesses {
        reply: oneshot::Sender<Result<Vec<ProcessRecord>, ReadError>>,
    },
    ClearProcess {
        exited: ExitConfirmed,
        reply: oneshot::Sender<Result<(), TxError<SqliteError>>>,
    },
    Stop {
        reply: oneshot::Sender<Result<(), StopError>>,
    },
}

/// Durable state owned by the core thread for the instance's lifetime.
#[derive(Debug)]
struct CoreState {
    store: Store,
    current: CurrentInstance,
}

/// Sends commands to the core thread. Consumed by [`CoreHandle::stop`].
#[derive(Debug)]
pub struct CoreHandle {
    commands: mpsc::Sender<Command>,
    thread: JoinHandle<()>,
}

/// Opens the store on a new core thread and commits the fence transaction.
/// Call only while holding the [`crate::fence::InstanceLock`].
pub fn start(db: PathBuf) -> Result<(CoreHandle, Ready), StartError> {
    let (ready_tx, ready_rx) = std_mpsc::sync_channel(1);
    let (commands, inbox) = mpsc::channel(COMMAND_QUEUE);
    let thread = thread::Builder::new()
        .name("uta-core".into())
        .spawn(move || run(db, ready_tx, inbox))
        .map_err(StartError::Spawn)?;
    match ready_rx.recv() {
        Ok(Ok(ready)) => Ok((CoreHandle { commands, thread }, ready)),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => {
            let _ = thread.join();
            Err(StartError::Vanished)
        }
    }
}

impl CoreHandle {
    /// Process-table rows registered by earlier instances (orphan candidates).
    pub async fn other_instance_processes(&self) -> Result<Vec<ProcessRecord>, CommandError> {
        let (reply, answer) = oneshot::channel();
        self.send(Command::OtherInstanceProcesses { reply }).await?;
        Ok(answer.await.map_err(|_| CommandError::CoreGone)??)
    }

    /// Clears a process-table row; the OS's exit confirmation is consumed.
    pub async fn clear_process(&self, exited: ExitConfirmed) -> Result<(), CommandError> {
        let (reply, answer) = oneshot::channel();
        self.send(Command::ClearProcess { exited, reply }).await?;
        Ok(answer.await.map_err(|_| CommandError::CoreGone)??)
    }

    /// Controlled stop, instance part (design §7.2 受控停止 steps 5–6 up to
    /// closing SQLite): end anchor, close, join the thread.
    pub async fn stop(self) -> Result<(), StopError> {
        let (reply, answer) = oneshot::channel();
        let sent = self.commands.send(Command::Stop { reply }).await;
        let result = match sent {
            Ok(()) => answer.await.unwrap_or(Err(StopError::Vanished)),
            Err(_) => Err(StopError::Vanished),
        };
        let thread = self.thread;
        let joined = tokio::task::spawn_blocking(move || thread.join()).await;
        match (result, joined) {
            (Ok(()), Ok(Ok(()))) => Ok(()),
            (Err(e), _) => Err(e),
            (Ok(()), _) => Err(StopError::Vanished),
        }
    }

    async fn send(&self, command: Command) -> Result<(), CommandError> {
        self.commands
            .send(command)
            .await
            .map_err(|_| CommandError::CoreGone)
    }
}

fn run(
    db: PathBuf,
    ready: std_mpsc::SyncSender<Result<Ready, StartError>>,
    mut inbox: mpsc::Receiver<Command>,
) {
    let state = match open(db) {
        Ok((state, report)) => {
            if ready.send(Ok(report)).is_err() {
                return;
            }
            state
        }
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    serve(state, &mut inbox);
}

fn open(db: PathBuf) -> Result<(CoreState, Ready), StartError> {
    let mut store = Store::open(&db)?;
    let current = store
        .transact(|tx| tx.instances().begin(UnixMillis::now()))
        .map_err(StartError::Fence)?
        .into_inner();
    let previous = store.previous_instance(&current)?;
    let ready = Ready {
        instance: current.id(),
        previous,
    };
    Ok((CoreState { store, current }, ready))
}

fn serve(mut state: CoreState, inbox: &mut mpsc::Receiver<Command>) {
    while let Some(command) = inbox.blocking_recv() {
        match command {
            Command::OtherInstanceProcesses { reply } => {
                let _ = reply.send(state.store.processes_of_other_instances(&state.current));
            }
            Command::ClearProcess { exited, reply } => {
                let result = state
                    .store
                    .transact(|tx| tx.processes().clear(exited))
                    .map(Committed::into_inner);
                let _ = reply.send(result);
            }
            Command::Stop { reply } => {
                let _ = reply.send(state.stop());
                return;
            }
        }
    }
    // Every handle is gone without a stop: the instance ends like a crash,
    // without an end anchor (design §7.2 "崩溃路径不变").
}

impl CoreState {
    fn stop(self) -> Result<(), StopError> {
        let CoreState { mut store, current } = self;
        store
            .transact(|tx| tx.instances().end(current, UnixMillis::now()))
            .map_err(StopError::EndAnchor)?
            .into_inner();
        store.close().map_err(StopError::Close)
    }
}
