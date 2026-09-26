use std::{future::pending, sync::Arc};

use serde_json::{Value, json};
use uta_channel::{Credential, take_inherited};
use uta_rpc::{FrameConfig, InboundMessage, PeerHandle, Request, Responder, RpcError};

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(_) => std::process::ExitCode::FAILURE,
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let inherited = take_inherited()?;
    let credential = Arc::new(inherited.credential);
    let (core, mut inbound, peer_task) = uta_rpc::start(inherited.channel, FrameConfig::default());
    let mut peer_task = tokio::spawn(peer_task);
    let mut handlers = tokio::task::JoinSet::new();

    let peer_end = loop {
        tokio::select! {
            result = &mut peer_task => break result?,
            message = inbound.recv() => {
                let Some(message) = message else {
                    break peer_task.await?;
                };
                if let InboundMessage::Request(request) = message {
                    handlers.spawn(serve_request(
                        request,
                        Arc::clone(&credential),
                        core.clone(),
                    ));
                }
            }
        }
    };

    // The peer has ended, so pending fixture handlers can no longer complete
    // useful work. Their responders are dropped only after the session closes.
    handlers.abort_all();
    while handlers.join_next().await.is_some() {}
    let _ = peer_end;
    Ok(())
}

async fn serve_request(request: Request, credential: Arc<Credential>, core: PeerHandle) {
    let (method, params, responder) = request.into_parts();
    match method.as_str() {
        "credential" => responder.respond(Ok(credential.expose())),
        "echo" => respond_with_params(params.as_ref(), responder),
        "call_back" => call_back(params.as_ref(), responder, core).await,
        "hang" => hang(params.as_ref(), responder, core).await,
        "environment" => respond_with_environment(responder),
        _ => respond_error(responder, rpc_error(-32601, "method not found")),
    }
}

fn respond_with_params(params: &[u8], responder: Responder) {
    match serde_json::from_slice::<Value>(params) {
        Ok(value) => responder.respond(Ok(&value)),
        Err(_) => respond_error(responder, rpc_error(-32602, "invalid parameters")),
    }
}

async fn call_back(params: &[u8], responder: Responder, core: PeerHandle) {
    let params = match serde_json::from_slice::<Value>(params) {
        Ok(value) => value,
        Err(_) => {
            respond_error(responder, rpc_error(-32602, "invalid parameters"));
            return;
        }
    };

    match core.call("ping", &params).await {
        Ok(response) => match serde_json::from_slice::<Value>(&response) {
            Ok(value) => responder.respond(Ok(&value)),
            Err(_) => respond_error(responder, rpc_error(-32603, "invalid callback response")),
        },
        Err(error) => respond_error(responder, rpc_error(-32603, &error.to_string())),
    }
}

async fn hang(params: &[u8], responder: Responder, core: PeerHandle) {
    let request_id = match serde_json::from_slice::<Value>(params)
        .ok()
        .and_then(|value| value.get("id").and_then(Value::as_u64))
    {
        Some(id) => id,
        None => {
            respond_error(responder, rpc_error(-32602, "hang requires an unsigned id"));
            return;
        }
    };

    // Keep the responder in this handler before acknowledging that this call is
    // pending. The test uses the correlated notification instead of a sleep.
    let _responder = responder;
    let _ = core
        .notify("hang_started", &json!({"id": request_id}))
        .await;
    pending::<()>().await;
}

fn respond_with_environment(responder: Responder) {
    let args = std::env::args_os()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let env = std::env::vars_os()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect::<Vec<_>>();
    let result = json!({"args": args, "env": env});
    responder.respond(Ok(&result));
}

fn respond_error(responder: Responder, error: RpcError) {
    responder.respond::<Value>(Err(error));
}

fn rpc_error(code: i64, message: &str) -> RpcError {
    RpcError {
        code,
        message: message.to_owned(),
        data: None,
    }
}
