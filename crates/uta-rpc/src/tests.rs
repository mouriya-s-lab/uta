use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde::ser::Error as _;
use serde::{Serialize, Serializer};
use tokio::io::{AsyncRead, AsyncWrite, DuplexStream, ReadBuf, duplex};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::{
    CallError, FrameConfig, InboundMessage, NotifyError, PeerEnd, RpcError, start,
};

fn is_clean_end(end: PeerEnd) -> bool {
    matches!(end, PeerEnd::LocalClosed | PeerEnd::RemoteClosed)
}


fn expect_bytes(result: Result<Bytes, CallError>) -> Bytes {
    match result {
        Ok(bytes) => bytes,
        Err(error) => panic!("expected successful call, got {error}"),
    }
}

#[derive(Debug)]
struct FailAfterWrite {
    inner: DuplexStream,
    wrote: bool,
}

impl AsyncRead for FailAfterWrite {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.wrote {
            return Poll::Ready(Err(io::Error::other("injected read failure")));
        }
        Pin::new(&mut this.inner).poll_read(context, buffer)
    }
}

impl AsyncWrite for FailAfterWrite {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let result = Pin::new(&mut this.inner).poll_write(context, buffer);
        if matches!(result, Poll::Ready(Ok(written)) if written > 0) {
            this.wrote = true;
        }
        result
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(context)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(context)
    }
}


#[tokio::test(flavor = "current_thread")]
async fn concurrent_calls_work_in_both_directions() {
    let (io_a, io_b) = duplex(8 * 1024);
    let (a, mut a_inbound, a_task) = start(io_a, FrameConfig::default());
    let (b, mut b_inbound, b_task) = start(io_b, FrameConfig::default());
    let a_task = tokio::spawn(a_task);
    let b_task = tokio::spawn(b_task);

    let a_call = {
        let a = a.clone();
        tokio::spawn(async move { a.call("from-a", &serde_json::json!({"side":"a"})).await })
    };
    let b_call = {
        let b = b.clone();
        tokio::spawn(async move { b.call("from-b", &serde_json::json!({"side":"b"})).await })
    };

    let request_at_a = a_inbound.recv().await.expect("request from b");
    let request_at_b = b_inbound.recv().await.expect("request from a");
    let InboundMessage::Request(request_at_a) = request_at_a else {
        panic!("expected request");
    };
    let InboundMessage::Request(request_at_b) = request_at_b else {
        panic!("expected request");
    };
    assert_eq!(request_at_a.method(), "from-b");
    assert_eq!(request_at_a.params().as_ref(), &br#"{"side":"b"}"#[..]);
    assert_eq!(request_at_b.method(), "from-a");
    assert_eq!(request_at_b.params().as_ref(), &br#"{"side":"a"}"#[..]);

    let a_result = serde_json::json!({"reply":"to-a"});
    let b_result = serde_json::json!({"reply":"to-b"});
    request_at_a
        .into_parts()
        .2
        .respond(Ok(&b_result));
    request_at_b
        .into_parts()
        .2
        .respond(Ok(&a_result));

    assert_eq!(
        expect_bytes(a_call.await.expect("a call task")),
        Bytes::from_static(br#"{"reply":"to-a"}"#)
    );
    assert_eq!(
        expect_bytes(b_call.await.expect("b call task")),
        Bytes::from_static(br#"{"reply":"to-b"}"#)
    );

    a.close();
    b.close();
    assert!(is_clean_end(a_task.await.expect("a peer task")));
    assert!(is_clean_end(b_task.await.expect("b peer task")));
}

#[tokio::test(flavor = "current_thread")]
async fn closing_completes_every_pending_call_once_and_new_operations_are_closed() {
    const CALLS: usize = 6;
    let (io_a, io_b) = duplex(16 * 1024);
    let (a, _a_inbound, a_task) = start(io_a, FrameConfig::default());
    let (b, mut b_inbound, b_task) = start(io_b, FrameConfig::default());
    let a_task = tokio::spawn(a_task);
    let b_task = tokio::spawn(b_task);

    let calls = (0..CALLS)
        .map(|_| {
            let a = a.clone();
            tokio::spawn(async move { a.call("pending", &()).await })
        })
        .collect::<Vec<_>>();
    let mut held_requests = Vec::with_capacity(CALLS);
    for _ in 0..CALLS {
        let Some(InboundMessage::Request(request)) = b_inbound.recv().await else {
            panic!("expected pending request");
        };
        held_requests.push(request);
    }

    a.close();
    assert!(matches!(a_task.await.expect("a peer task"), PeerEnd::LocalClosed));
    for call in calls {
        assert!(matches!(call.await.expect("call task"), Err(CallError::Closed)));
    }
    assert!(matches!(a.call("after-close", &()).await, Err(CallError::Closed)));
    assert!(matches!(a.notify("after-close", &()).await, Err(NotifyError::Closed)));
    b.close();
    assert!(is_clean_end(b_task.await.expect("b peer task")));
    drop(held_requests);
}

#[tokio::test(flavor = "current_thread")]
async fn remote_close_completes_pending_call_closed() {
    let (peer_io, remote_io) = duplex(8 * 1024);
    let (peer, _inbound, peer_task) = start(peer_io, FrameConfig::default());
    let peer_task = tokio::spawn(peer_task);
    let mut remote = Framed::new(remote_io, LengthDelimitedCodec::new());
    let call = {
        let peer = peer.clone();
        tokio::spawn(async move { peer.call("pending", &()).await })
    };
    let _outgoing = remote.next().await.expect("outgoing call").expect("valid frame");
    drop(remote);

    assert!(matches!(call.await.expect("call task"), Err(CallError::Closed)));
    assert!(matches!(peer_task.await.expect("peer task"), PeerEnd::RemoteClosed));
}

#[tokio::test(flavor = "current_thread")]
async fn io_error_completes_pending_call_closed() {
    let (peer_io, _remote_io) = duplex(8 * 1024);
    let failing_io = FailAfterWrite {
        inner: peer_io,
        wrote: false,
    };
    let (peer, _inbound, peer_task) = start(failing_io, FrameConfig::default());
    let peer_task = tokio::spawn(peer_task);
    let call = {
        let peer = peer.clone();
        tokio::spawn(async move { peer.call("pending", &()).await })
    };

    assert!(matches!(call.await.expect("call task"), Err(CallError::Closed)));
    let PeerEnd::Io(error) = peer_task.await.expect("peer task") else {
        panic!("expected injected I/O error");
    };
    assert_eq!(error.kind(), io::ErrorKind::Other);
}

#[tokio::test(flavor = "current_thread")]
async fn dropped_responder_returns_exact_internal_error() {
    let (io_a, io_b) = duplex(8 * 1024);
    let (a, _a_inbound, a_task) = start(io_a, FrameConfig::default());
    let (b, mut b_inbound, b_task) = start(io_b, FrameConfig::default());
    let a_task = tokio::spawn(a_task);
    let b_task = tokio::spawn(b_task);

    let call = {
        let a = a.clone();
        tokio::spawn(async move { a.call("will-drop", &()).await })
    };
    let Some(InboundMessage::Request(request)) = b_inbound.recv().await else {
        panic!("expected request");
    };
    drop(request);

    let result = call.await.expect("call task");
    let Err(CallError::Remote(error)) = result else {
        panic!("expected remote internal error");
    };
    assert_eq!(error.code, -32603);
    assert_eq!(error.message, "request dropped without a response");
    assert_eq!(error.data, None);

    a.close();
    b.close();
    assert!(is_clean_end(a_task.await.expect("a peer task")));
    assert!(is_clean_end(b_task.await.expect("b peer task")));
}

#[tokio::test(flavor = "current_thread")]
async fn malformed_frame_ends_peer_and_closes_pending_call() {
    let (peer_io, raw_io) = duplex(8 * 1024);
    let (peer, _inbound, peer_task) = start(peer_io, FrameConfig::default());
    let peer_task = tokio::spawn(peer_task);
    let mut raw = Framed::new(raw_io, LengthDelimitedCodec::new());
    let call = {
        let peer = peer.clone();
        tokio::spawn(async move { peer.call("pending", &()).await })
    };

    let outgoing = raw.next().await.expect("outgoing call").expect("valid frame");
    assert_eq!(serde_json::from_slice::<serde_json::Value>(&outgoing).expect("call JSON")["id"], 1);
    raw.send(Bytes::from_static(b"not JSON")).await.expect("send malformed frame");

    assert!(matches!(call.await.expect("call task"), Err(CallError::Closed)));
    let end = peer_task.await.expect("peer task");
    assert!(matches!(end, PeerEnd::Protocol(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn oversize_frame_is_a_protocol_end_and_closes_pending_call() {
    let (peer_io, raw_io) = duplex(8 * 1024);
    let config = FrameConfig { max_frame_len: 64 };
    let (peer, _inbound, peer_task) = start(peer_io, config);
    let peer_task = tokio::spawn(peer_task);
    let mut raw = Framed::new(raw_io, LengthDelimitedCodec::new());
    let call = {
        let peer = peer.clone();
        tokio::spawn(async move { peer.call("pending", &()).await })
    };
    let _outgoing = raw.next().await.expect("outgoing call").expect("valid frame");
    raw.send(Bytes::from(vec![b'x'; 65]))
        .await
        .expect("send oversized frame");

    assert!(matches!(call.await.expect("call task"), Err(CallError::Closed)));
    assert!(matches!(peer_task.await.expect("peer task"), PeerEnd::Protocol(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn notifications_are_delivered_without_a_response() {
    let (io_a, io_b) = duplex(8 * 1024);
    let (a, _a_inbound, a_task) = start(io_a, FrameConfig::default());
    let (b, mut b_inbound, b_task) = start(io_b, FrameConfig::default());
    let a_task = tokio::spawn(a_task);
    let b_task = tokio::spawn(b_task);

    a.notify("changed", &serde_json::json!({"revision":7}))
        .await
        .expect("notification send");
    let Some(InboundMessage::Notification(notification)) = b_inbound.recv().await else {
        panic!("expected notification");
    };
    assert_eq!(notification.method(), "changed");
    assert_eq!(notification.params().as_ref(), &br#"{"revision":7}"#[..]);

    a.close();
    b.close();
    assert!(is_clean_end(a_task.await.expect("a peer task")));
    assert!(is_clean_end(b_task.await.expect("b peer task")));
}

#[tokio::test(flavor = "current_thread")]
async fn request_ids_echo_verbatim_and_missing_params_default_to_null() {
    let (peer_io, raw_io) = duplex(8 * 1024);
    let (peer, mut inbound, peer_task) = start(peer_io, FrameConfig::default());
    let peer_task = tokio::spawn(peer_task);
    let mut raw = Framed::new(raw_io, LengthDelimitedCodec::new());

    raw.send(Bytes::from_static(
        br#"{"jsonrpc":"2.0","method":"echo","id":"x\u002fy"}"#,
    ))
    .await
    .expect("send request");
    let Some(InboundMessage::Request(request)) = inbound.recv().await else {
        panic!("expected request");
    };
    assert_eq!(request.method(), "echo");
    assert_eq!(request.params().as_ref(), &b"null"[..]);
    request.into_parts().2.respond(Ok(&"ok"));

    let response = raw.next().await.expect("response").expect("valid response frame");
    let text = std::str::from_utf8(&response).expect("response is UTF-8");
    assert!(text.contains(r#""id":"x\u002fy""#), "response was {text}");
    assert!(text.contains(r#""result":"ok""#), "response was {text}");

    raw.send(Bytes::from_static(
        br#"{"jsonrpc":"2.0","method":"number-id","id":1.00}"#,
    ))
    .await
    .expect("send numeric-id request");
    let Some(InboundMessage::Request(request)) = inbound.recv().await else {
        panic!("expected numeric-id request");
    };
    assert_eq!(request.params().as_ref(), &b"null"[..]);
    request.into_parts().2.respond(Ok(&true));
    let response = raw.next().await.expect("numeric-id response").expect("valid response frame");
    let text = std::str::from_utf8(&response).expect("response is UTF-8");
    assert!(text.contains(r#""id":1.00"#), "response was {text}");
    assert!(text.contains(r#""result":true"#), "response was {text}");

    raw.send(Bytes::from_static(
        br#"{"jsonrpc":"2.0","method":"tick"}"#,
    ))
    .await
    .expect("send notification without params");
    let Some(InboundMessage::Notification(notification)) = inbound.recv().await else {
        panic!("expected notification");
    };
    assert_eq!(notification.method(), "tick");
    assert_eq!(notification.params().as_ref(), &b"null"[..]);
    peer.close();
    assert!(matches!(peer_task.await.expect("peer task"), PeerEnd::LocalClosed));
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_response_id_and_non_message_are_protocol_violations() {
    for input in [
        br#"{"jsonrpc":"2.0","id":90,"result":null}"#.as_slice(),
        br#"{"jsonrpc":"2.0"}"#.as_slice(),
    ] {
        let (peer_io, raw_io) = duplex(8 * 1024);
        let (peer, _inbound, peer_task) = start(peer_io, FrameConfig::default());
        let peer_task = tokio::spawn(peer_task);
        let mut raw = Framed::new(raw_io, LengthDelimitedCodec::new());
        let call = {
            let peer = peer.clone();
            tokio::spawn(async move { peer.call("pending", &()).await })
        };
        let _outgoing = raw.next().await.expect("outgoing call").expect("valid frame");
        raw.send(Bytes::copy_from_slice(input))
            .await
            .expect("send invalid message");
        assert!(matches!(call.await.expect("call task"), Err(CallError::Closed)));
        assert!(matches!(peer_task.await.expect("peer task"), PeerEnd::Protocol(_)));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn serialization_failures_send_nothing_and_leave_peer_running() {
    struct CannotSerialize;
    impl Serialize for CannotSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(S::Error::custom("intentional encoding failure"))
        }
    }

    let (io_a, io_b) = duplex(8 * 1024);
    let (a, _a_inbound, a_task) = start(io_a, FrameConfig::default());
    let (b, mut b_inbound, b_task) = start(io_b, FrameConfig::default());
    let a_task = tokio::spawn(a_task);
    let b_task = tokio::spawn(b_task);

    assert!(matches!(a.call("bad", &CannotSerialize).await, Err(CallError::Encode(_))));
    assert!(matches!(a.notify("bad", &CannotSerialize).await, Err(NotifyError::Encode(_))));

    let call = {
        let a = a.clone();
        tokio::spawn(async move { a.call("good", &()).await })
    };
    let Some(InboundMessage::Request(request)) = b_inbound.recv().await else {
        panic!("expected the valid call after encode errors");
    };
    assert_eq!(request.method(), "good");
    request.into_parts().2.respond(Ok(&"still-running"));
    assert_eq!(
        expect_bytes(call.await.expect("call task")),
        Bytes::from_static(br#""still-running""#)
    );

    a.close();
    b.close();
    assert!(matches!(a_task.await.expect("a peer task"), PeerEnd::LocalClosed));
    assert!(is_clean_end(b_task.await.expect("b peer task")));
}

#[tokio::test(flavor = "current_thread")]
async fn received_params_and_results_share_their_frame_allocation() {
    fn assert_is_slice(frame: &Bytes, slice: &Bytes) {
        let frame_start = frame.as_ptr() as usize;
        let frame_end = frame_start + frame.len();
        let slice_start = slice.as_ptr() as usize;
        let slice_end = slice_start + slice.len();
        assert!(slice_start >= frame_start && slice_end <= frame_end);
    }

    let (writer_io, reader_io) = duplex(8 * 1024);
    let mut writer = Framed::new(writer_io, LengthDelimitedCodec::new());
    let mut reader = Framed::new(reader_io, LengthDelimitedCodec::new());

    writer
        .send(Bytes::from_static(
            br#"{"jsonrpc":"2.0","method":"work","id":"request-7","params":{"a":[1,2]}}"#,
        ))
        .await
        .expect("send request frame");
    let request_frame = reader
        .next()
        .await
        .expect("request frame")
        .expect("valid request frame")
        .freeze();
    let crate::ParsedFrame::Request { params, .. } =
        crate::parse_frame(&request_frame).expect("request parse")
    else {
        panic!("expected parsed request");
    };
    assert_is_slice(&request_frame, &params);

    writer
        .send(Bytes::from_static(
            br#"{"jsonrpc":"2.0","id":1,"result":{"b":[3,4]}}"#,
        ))
        .await
        .expect("send response frame");
    let result_frame = reader
        .next()
        .await
        .expect("response frame")
        .expect("valid response frame")
        .freeze();
    let crate::ParsedFrame::Response {
        result: Ok(result), ..
    } = crate::parse_frame(&result_frame).expect("response parse")
    else {
        panic!("expected parsed response");
    };
    assert_is_slice(&result_frame, &result);
}

#[tokio::test(flavor = "current_thread")]
async fn remote_error_data_is_kept_as_raw_json_bytes() {
    let (writer_io, reader_io) = duplex(8 * 1024);
    let mut writer = Framed::new(writer_io, LengthDelimitedCodec::new());
    let mut reader = Framed::new(reader_io, LengthDelimitedCodec::new());
    writer
        .send(Bytes::from_static(
            br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32001,"message":"failed","data":{"retry":false}}}"#,
        ))
        .await
        .expect("send error response frame");
    let frame = reader
        .next()
        .await
        .expect("error frame")
        .expect("valid error frame")
        .freeze();
    let crate::ParsedFrame::Response {
        result: Err(error), ..
    } = crate::parse_frame(&frame).expect("error parse")
    else {
        panic!("expected parsed error");
    };
    assert_eq!(
        error,
        RpcError {
            code: -32001,
            message: "failed".to_owned(),
            data: Some(Bytes::from_static(br#"{"retry":false}"#)),
        }
    );
    let data = error.data.as_ref().expect("error data");
    let frame_start = frame.as_ptr() as usize;
    let frame_end = frame_start + frame.len();
    let data_start = data.as_ptr() as usize;
    let data_end = data_start + data.len();
    assert!(data_start >= frame_start && data_end <= frame_end);
}
