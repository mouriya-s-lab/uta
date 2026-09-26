//! Symmetric JSON-RPC 2.0 peers over length-delimited Tokio transports.
//!
//! A [`PeerTask`] owns the framed transport and all pending calls. The owner must
//! poll or spawn the task, and drain [`Inbound`] concurrently with its own calls;
//! inbound delivery uses a bounded channel and can backpressure the peer.

use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Bytes;
use futures_util::{Sink, Stream};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{mpsc, oneshot},
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

const INBOUND_CAPACITY: usize = 32;
const DEFAULT_MAX_FRAME_LEN: usize = 16 * 1024 * 1024;

/// Length limit for an encoded frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameConfig {
    /// Maximum payload size, excluding the four-byte length prefix.
    pub max_frame_len: usize,
}

impl Default for FrameConfig {
    fn default() -> Self {
        Self {
            max_frame_len: DEFAULT_MAX_FRAME_LEN,
        }
    }
}

/// A peer has stopped because of transport closure, local closure, or a protocol
/// violation. Calls which were still pending at that point complete with
/// [`CallError::Closed`].
#[derive(Debug)]
pub enum PeerEnd {
    /// The other side cleanly closed its transport.
    RemoteClosed,
    /// The local handle requested closure or its command channel was dropped.
    LocalClosed,
    /// A frame or JSON-RPC message violated the protocol.
    Protocol(ProtocolViolation),
    /// The underlying transport failed.
    Io(io::Error),
}

/// A JSON-RPC protocol violation, including malformed or oversized frames.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct ProtocolViolation {
    message: String,
}

impl ProtocolViolation {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the explanation for this protocol violation.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// The result of a call to the remote peer.
#[derive(Debug, Error)]
pub enum CallError {
    /// The peer ended before a response arrived.
    #[error("peer is closed")]
    Closed,
    /// The remote peer returned a JSON-RPC error.
    #[error("remote JSON-RPC error {0}")]
    Remote(RpcError),
    /// The call parameters could not be encoded as JSON; no request was sent.
    #[error("failed to encode JSON-RPC parameters: {0}")]
    Encode(#[source] serde_json::Error),
}

/// The result of sending a notification.
#[derive(Debug, Error)]
pub enum NotifyError {
    /// The peer ended before the notification was sent.
    #[error("peer is closed")]
    Closed,
    /// The notification parameters could not be encoded as JSON; no message was sent.
    #[error("failed to encode JSON-RPC parameters: {0}")]
    Encode(#[source] serde_json::Error),
}

/// A JSON-RPC error returned by the remote side or sent by a responder.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("JSON-RPC error {code}: {message}")]
pub struct RpcError {
    /// JSON-RPC error code.
    pub code: i64,
    /// Human-readable error message.
    pub message: String,
    /// Optional raw JSON error data.
    pub data: Option<Bytes>,
}

/// Cloneable sender for calls and notifications to the peer task.
#[derive(Clone, Debug)]
pub struct PeerHandle {
    commands: mpsc::UnboundedSender<Command>,
}

impl PeerHandle {
    /// Sends a JSON-RPC request and resolves with its raw JSON result.
    pub async fn call<P: Serialize + ?Sized>(
        &self,
        method: &str,
        params: &P,
    ) -> Result<Bytes, CallError> {
        if self.commands.is_closed() {
            return Err(CallError::Closed);
        }
        let params = serde_json::to_vec(params).map_err(CallError::Encode)?;
        let (completion, response) = oneshot::channel();
        if self
            .commands
            .send(Command::Call {
                method: method.to_owned(),
                params: Bytes::from(params),
                completion,
            })
            .is_err()
        {
            return Err(CallError::Closed);
        }
        response.await.unwrap_or(Err(CallError::Closed))
    }

    /// Sends a JSON-RPC notification, which has no request ID or response.
    pub async fn notify<P: Serialize + ?Sized>(
        &self,
        method: &str,
        params: &P,
    ) -> Result<(), NotifyError> {
        if self.commands.is_closed() {
            return Err(NotifyError::Closed);
        }
        let params = serde_json::to_vec(params).map_err(NotifyError::Encode)?;
        let frame = encode_notification(method, &params);
        let (completion, sent) = oneshot::channel();
        if self
            .commands
            .send(Command::Notify { frame, completion })
            .is_err()
        {
            return Err(NotifyError::Closed);
        }
        sent.await.map_err(|_| NotifyError::Closed)
    }

    /// Ends the peer. All calls pending when the task observes closure complete
    /// with [`CallError::Closed`].
    pub fn close(&self) {
        let _ = self.commands.send(Command::Close);
    }
}

/// Bounded receiver for requests and notifications received from the peer.
///
/// The owner must drain this concurrently with awaiting its own calls, so an
/// inbound message cannot fill the bounded queue and stall response processing.
pub struct Inbound {
    messages: mpsc::Receiver<InboundMessage>,
}

impl Inbound {
    /// Receives the next request or notification, or `None` after the peer ends.
    pub async fn recv(&mut self) -> Option<InboundMessage> {
        self.messages.recv().await
    }
}

impl std::fmt::Debug for Inbound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Inbound").finish_non_exhaustive()
    }
}

/// A request delivered to the local application.
pub struct Request {
    method: String,
    params: Bytes,
    responder: Responder,
}

impl Request {
    /// Returns the request method.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Returns the raw JSON parameters; absent parameters are represented as `null`.
    pub fn params(&self) -> &Bytes {
        &self.params
    }

    /// Splits the request into its method, raw parameters, and one-shot responder.
    pub fn into_parts(self) -> (String, Bytes, Responder) {
        (self.method, self.params, self.responder)
    }
}

impl std::fmt::Debug for Request {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Request")
            .field("method", &self.method)
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

/// A notification delivered to the local application.
#[derive(Debug)]
pub struct Notification {
    method: String,
    params: Bytes,
}

impl Notification {
    /// Returns the notification method.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Returns the raw JSON parameters; absent parameters are represented as `null`.
    pub fn params(&self) -> &Bytes {
        &self.params
    }

    /// Splits the notification into its method and raw parameters.
    pub fn into_parts(self) -> (String, Bytes) {
        (self.method, self.params)
    }
}

/// The sole responder for an inbound request.
///
/// Dropping it without calling [`Responder::respond`] sends JSON-RPC error
/// `-32603` with message `request dropped without a response`.
#[must_use = "dropping a request responder sends an internal JSON-RPC error"]
pub struct Responder {
    id: Option<Bytes>,
    commands: mpsc::UnboundedSender<Command>,
}

impl Responder {
    /// Sends one JSON-RPC result or error for this request.
    pub fn respond<R: Serialize + ?Sized>(mut self, response: Result<&R, RpcError>) {
        let Some(id) = self.id.take() else {
            return;
        };
        let response = match response {
            Ok(result) => match serde_json::to_vec(result) {
                Ok(result) => ResponseBody::Result(Bytes::from(result)),
                Err(_) => ResponseBody::Error(RpcError {
                    code: -32603,
                    message: "failed to serialize response".to_owned(),
                    data: None,
                }),
            },
            Err(error) => ResponseBody::Error(error),
        };
        let _ = self.commands.send(Command::Respond { id, response });
    }
}

impl std::fmt::Debug for Responder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Responder").finish_non_exhaustive()
    }
}

impl Drop for Responder {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            let _ = self.commands.send(Command::Respond {
                id,
                response: ResponseBody::Error(RpcError {
                    code: -32603,
                    message: "request dropped without a response".to_owned(),
                    data: None,
                }),
            });
        }
    }
}

/// Message delivered from the remote peer.
#[derive(Debug)]
pub enum InboundMessage {
    /// A request that must be answered exactly once by its responder.
    Request(Request),
    /// A notification which has no responder.
    Notification(Notification),
}

/// A future which drives one peer. It runs only while polled.
#[must_use = "the peer runs only while this future is polled"]
pub struct PeerTask {
    future: Pin<Box<dyn Future<Output = PeerEnd> + Send>>,
}

impl Future for PeerTask {
    type Output = PeerEnd;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().future.as_mut().poll(context)
    }
}

impl std::fmt::Debug for PeerTask {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("PeerTask").finish_non_exhaustive()
    }
}

/// Starts a symmetric JSON-RPC 2.0 peer over `io`.
///
/// The peer task is the only owner of the framed transport and pending-call map.
///
/// # Panics
///
/// The codec builder panics if `max_frame_len` cannot be represented by its
/// configured four-byte length field.
pub fn start<T>(io: T, config: FrameConfig) -> (PeerHandle, Inbound, PeerTask)
where
    T: AsyncRead + AsyncWrite + Send + 'static,
{
    let codec = LengthDelimitedCodec::builder()
        .max_frame_length(config.max_frame_len)
        .new_codec();
    let framed = Framed::new(io, codec);
    let (commands, command_rx) = mpsc::unbounded_channel();
    let responder_commands = commands.clone();
    let (messages, inbound_rx) = mpsc::channel(INBOUND_CAPACITY);
    let handle = PeerHandle { commands };
    let inbound = Inbound {
        messages: inbound_rx,
    };
    let task = PeerTask {
        future: Box::pin(run_peer(
            framed,
            command_rx,
            responder_commands,
            messages,
        )),
    };
    (handle, inbound, task)
}

#[derive(Debug)]
enum Command {
    Call {
        method: String,
        params: Bytes,
        completion: oneshot::Sender<Result<Bytes, CallError>>,
    },
    Notify {
        frame: Bytes,
        completion: oneshot::Sender<()>,
    },
    Respond {
        id: Bytes,
        response: ResponseBody,
    },
    Close,
}

#[derive(Debug)]
enum ResponseBody {
    Result(Bytes),
    Error(RpcError),
}

struct PendingOutbound {
    frame: Bytes,
    flushed: Option<oneshot::Sender<()>>,
}

enum TransportEvent {
    Frame(Bytes),
    Eof,
    WriteProgress,
    Flushed,
}

async fn run_peer<T>(
    framed: Framed<T, LengthDelimitedCodec>,
    mut commands: mpsc::UnboundedReceiver<Command>,
    responder_commands: mpsc::UnboundedSender<Command>,
    inbound: mpsc::Sender<InboundMessage>,
) -> PeerEnd
where
    T: AsyncRead + AsyncWrite + Send + 'static,
{
    tokio::pin!(framed);
    let mut pending = HashMap::<u64, oneshot::Sender<Result<Bytes, CallError>>>::new();
    let mut outbound = VecDeque::<PendingOutbound>::new();
    let mut flush_waiters = Vec::<oneshot::Sender<()>>::new();
    let mut flush_pending = false;
    let mut next_id = Some(1_u64);

    let end = loop {
        tokio::select! {
            command = commands.recv() => {
                match command {
                    Some(Command::Call { method, params, completion }) => {
                        let Some(id) = next_id else {
                            break PeerEnd::Protocol(ProtocolViolation::new("request id space exhausted"));
                        };
                        next_id = id.checked_add(1);
                        pending.insert(id, completion);
                        outbound.push_back(PendingOutbound {
                            frame: encode_request(id, &method, &params),
                            flushed: None,
                        });
                    }
                    Some(Command::Notify { frame, completion }) => {
                        outbound.push_back(PendingOutbound {
                            frame,
                            flushed: Some(completion),
                        });
                    }
                    Some(Command::Respond { id, response }) => {
                        outbound.push_back(PendingOutbound {
                            frame: encode_response(&id, response),
                            flushed: None,
                        });
                    }
                    Some(Command::Close) | None => break PeerEnd::LocalClosed,
                }
            }
            result = futures_util::future::poll_fn(|context| {
                poll_transport(
                    framed.as_mut(),
                    &mut outbound,
                    &mut flush_waiters,
                    &mut flush_pending,
                    context,
                )
            }), if !outbound.is_empty() || flush_pending || !commands.is_closed() => {
                match result {
                    Ok(TransportEvent::Frame(frame)) => {
                        let parsed = match parse_frame(&frame) {
                            Ok(parsed) => parsed,
                            Err(violation) => break PeerEnd::Protocol(violation),
                        };
                        match parsed {
                            ParsedFrame::Response { id, result } => {
                                let Some(sender) = pending.remove(&id) else {
                                    break PeerEnd::Protocol(ProtocolViolation::new(format!("response for unknown request id {id}")));
                                };
                                let result = match result {
                                    Ok(result) => Ok(result),
                                    Err(error) => Err(CallError::Remote(error)),
                                };
                                let _ = sender.send(result);
                            }
                            ParsedFrame::Request { id, method, params } => {
                                let Some(id) = id else {
                                    if inbound.send(InboundMessage::Notification(Notification { method, params })).await.is_err() {
                                        break PeerEnd::LocalClosed;
                                    }
                                    continue;
                                };
                                let responder = Responder {
                                    id: Some(id),
                                    commands: responder_commands.clone(),
                                };
                                if inbound.send(InboundMessage::Request(Request { method, params, responder })).await.is_err() {
                                    break PeerEnd::LocalClosed;
                                }
                            }
                        }
                    }
                    Ok(TransportEvent::Eof) => break PeerEnd::RemoteClosed,
                    Ok(TransportEvent::WriteProgress) => {}
                    Ok(TransportEvent::Flushed) => {
                        for completion in flush_waiters.drain(..) {
                            let _ = completion.send(());
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                        break PeerEnd::Protocol(ProtocolViolation::new(format!("invalid or oversized frame: {error}")));
                    }
                    Err(error) => break PeerEnd::Io(error),
                }
            }
        }
    };

    for (_, completion) in pending.drain() {
        let _ = completion.send(Err(CallError::Closed));
    }
    end
}

fn poll_transport<T>(
    mut framed: Pin<&mut Framed<T, LengthDelimitedCodec>>,
    outbound: &mut VecDeque<PendingOutbound>,
    flush_waiters: &mut Vec<oneshot::Sender<()>>,
    flush_pending: &mut bool,
    context: &mut Context<'_>,
) -> Poll<io::Result<TransportEvent>>
where
    T: AsyncRead + AsyncWrite,
{
    if !outbound.is_empty() {
        match Sink::poll_ready(framed.as_mut(), context) {
            Poll::Pending => {}
            Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
            Poll::Ready(Ok(())) => {
                let Some(frame) = outbound.pop_front() else {
                    return Poll::Pending;
                };
                let PendingOutbound { frame, flushed } = frame;
                if let Err(error) = Sink::start_send(framed.as_mut(), frame) {
                    return Poll::Ready(Err(error));
                }
                if let Some(flushed) = flushed {
                    flush_waiters.push(flushed);
                }
                *flush_pending = true;
                return Poll::Ready(Ok(TransportEvent::WriteProgress));
            }
        }
    }

    if outbound.is_empty() && *flush_pending {
        match Sink::poll_flush(framed.as_mut(), context) {
            Poll::Pending => {}
            Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
            Poll::Ready(Ok(())) => {
                *flush_pending = false;
                return Poll::Ready(Ok(TransportEvent::Flushed));
            }
        }
    }

    match Stream::poll_next(framed.as_mut(), context) {
        Poll::Pending => Poll::Pending,
        Poll::Ready(None) => Poll::Ready(Ok(TransportEvent::Eof)),
        Poll::Ready(Some(Err(error))) => Poll::Ready(Err(error)),
        Poll::Ready(Some(Ok(frame))) => Poll::Ready(Ok(TransportEvent::Frame(frame.freeze()))),
    }
}

enum ParsedFrame {
    Request {
        id: Option<Bytes>,
        method: String,
        params: Bytes,
    },
    Response {
        id: u64,
        result: Result<Bytes, RpcError>,
    },
}

#[derive(Deserialize)]
struct RawMessage<'a> {
    #[serde(borrow)]
    jsonrpc: Option<&'a str>,
    #[serde(borrow)]
    method: Option<&'a str>,
    #[serde(borrow)]
    id: Option<&'a RawValue>,
    #[serde(borrow)]
    params: Option<&'a RawValue>,
    #[serde(borrow)]
    result: Option<&'a RawValue>,
    #[serde(borrow)]
    error: Option<RawRpcError<'a>>,
}

#[derive(Deserialize)]
struct RawRpcError<'a> {
    code: i64,
    #[serde(borrow)]
    message: &'a str,
    #[serde(borrow)]
    data: Option<&'a RawValue>,
}

fn parse_frame(frame: &Bytes) -> Result<ParsedFrame, ProtocolViolation> {
    let text = std::str::from_utf8(frame)
        .map_err(|error| ProtocolViolation::new(format!("frame is not UTF-8 JSON: {error}")))?;
    let message: RawMessage<'_> = serde_json::from_str(text)
        .map_err(|error| ProtocolViolation::new(format!("malformed JSON-RPC message: {error}")))?;
    if message.jsonrpc != Some("2.0") {
        return Err(ProtocolViolation::new("message is missing JSON-RPC version 2.0"));
    }

    match (message.method, message.id, message.params, message.result, message.error) {
        (Some(method), id, params, None, None) => {
            let id = id
                .map(|id| {
                    validate_request_id(id)?;
                    Ok::<Bytes, ProtocolViolation>(frame.slice_ref(id.get().as_bytes()))
                })
                .transpose()?;
            let params = params
                .map(|params| frame.slice_ref(params.get().as_bytes()))
                .unwrap_or_else(|| Bytes::from_static(b"null"));
            Ok(ParsedFrame::Request {
                id,
                method: method.to_owned(),
                params,
            })
        }
        (None, Some(id), _params, result, response_error) => {
            let id = parse_response_id(id)?;
            match (result, response_error) {
                (Some(result), None) => Ok(ParsedFrame::Response {
                    id,
                    result: Ok(frame.slice_ref(result.get().as_bytes())),
                }),
                (None, Some(error)) => {
                    let data = error
                        .data
                        .map(|data| frame.slice_ref(data.get().as_bytes()));
                    Ok(ParsedFrame::Response {
                        id,
                        result: Err(RpcError {
                            code: error.code,
                            message: error.message.to_owned(),
                            data,
                        }),
                    })
                }
                _ => Err(ProtocolViolation::new(
                    "response must contain exactly one of result or error",
                )),
            }
        }
        _ => Err(ProtocolViolation::new(
            "message is neither a request, notification, nor response",
        )),
    }
}

fn validate_request_id(id: &RawValue) -> Result<(), ProtocolViolation> {
    let first = id
        .get()
        .as_bytes()
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace());
    match first {
        Some(b'"' | b'-' | b'0'..=b'9') => Ok(()),
        _ => Err(ProtocolViolation::new(
            "request id must be a JSON number or string",
        )),
    }
}

fn parse_response_id(id: &RawValue) -> Result<u64, ProtocolViolation> {
    serde_json::from_str(id.get())
        .map_err(|_| ProtocolViolation::new("response id does not match an outgoing request id"))
}

fn encode_request(id: u64, method: &str, params: &[u8]) -> Bytes {
    let mut encoded = Vec::with_capacity(method.len() + params.len() + 48);
    encoded.extend_from_slice(br#"{"jsonrpc":"2.0","method":"#);
    write_json(&mut encoded, method);
    encoded.extend_from_slice(br#","params":"#);
    encoded.extend_from_slice(params);
    encoded.extend_from_slice(br#","id":"#);
    write_json(&mut encoded, &id);
    encoded.push(b'}');
    Bytes::from(encoded)
}

fn encode_notification(method: &str, params: &[u8]) -> Bytes {
    let mut encoded = Vec::with_capacity(method.len() + params.len() + 40);
    encoded.extend_from_slice(br#"{"jsonrpc":"2.0","method":"#);
    write_json(&mut encoded, method);
    encoded.extend_from_slice(br#","params":"#);
    encoded.extend_from_slice(params);
    encoded.push(b'}');
    Bytes::from(encoded)
}

fn encode_response(id: &Bytes, response: ResponseBody) -> Bytes {
    let mut encoded = Vec::with_capacity(id.len() + 96);
    encoded.extend_from_slice(br#"{"jsonrpc":"2.0","id":"#);
    encoded.extend_from_slice(id);
    match response {
        ResponseBody::Result(result) => {
            encoded.extend_from_slice(br#","result":"#);
            encoded.extend_from_slice(&result);
            encoded.push(b'}');
        }
        ResponseBody::Error(error) => {
            let error = if error
                .data
                .as_ref()
                .is_some_and(|data| !is_raw_json(data))
            {
                RpcError {
                    code: -32603,
                    message: "invalid JSON-RPC error data".to_owned(),
                    data: None,
                }
            } else {
                error
            };
            encoded.extend_from_slice(br#","error":{"code":"#);
            write_json(&mut encoded, &error.code);
            encoded.extend_from_slice(br#","message":"#);
            write_json(&mut encoded, &error.message);
            if let Some(data) = error.data {
                encoded.extend_from_slice(br#","data":"#);
                encoded.extend_from_slice(&data);
            }
            encoded.extend_from_slice(b"}}");
        }
    }
    Bytes::from(encoded)
}

fn is_raw_json(bytes: &Bytes) -> bool {
    std::str::from_utf8(bytes)
        .ok()
        .is_some_and(|text| serde_json::from_str::<&RawValue>(text).is_ok())
}

fn write_json<T: Serialize + ?Sized>(output: &mut Vec<u8>, value: &T) {
    // The only values passed here are strings and integers, and Vec writes cannot fail.
    let _ = serde_json::to_writer(output, value);
}

#[cfg(test)]
mod tests;
