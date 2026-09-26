//! A core-created, single-child session transport and one-shot credential delivery.
//!
//! Only the selected child inherits the transport and credential ends. Dropping the
//! core's [`SessionChannel`] closes that end of the session; credentials are never
//! passed as command-line arguments or environment values.

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::ExitStatus;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use zeroize::Zeroizing;

pub(crate) const SESSION_CHANNEL_ENV: &str = "UTA_SESSION_CHANNEL";
pub(crate) const CREDENTIAL_ENV: &str = "UTA_CREDENTIAL";

#[cfg(unix)]
#[path = "unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "windows.rs"]
mod platform;

/// The program and working directory for one child process.
#[derive(Debug)]
pub struct ChildSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: Option<PathBuf>,
}

/// Secret bytes owned by the caller until passed to the child's one-shot pipe.
pub struct Credential(Zeroizing<Vec<u8>>);

impl From<Vec<u8>> for Credential {
    fn from(value: Vec<u8>) -> Self {
        Self(Zeroizing::new(value))
    }
}

impl Credential {
    /// Borrow the credential without transferring or copying its bytes.
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Credential([redacted])")
    }
}

/// The session and OS process created together by the core.
#[derive(Debug)]
#[must_use = "the caller must own both the session and child process"]
pub struct Spawned {
    pub channel: SessionChannel,
    pub process: ChildProcess,
}

/// Bidirectional byte transport; dropping it closes the core's end.
#[derive(Debug)]
pub struct SessionChannel(pub(crate) platform::Channel);

impl AsyncRead for SessionChannel {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for SessionChannel {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

/// Owned OS process handle. Call [`ChildProcess::wait`] to reap the child.
#[derive(Debug)]
#[must_use = "the child process must be reaped"]
pub struct ChildProcess(pub(crate) platform::Process);

impl ChildProcess {
    pub fn pid(&self) -> u32 {
        self.0.pid()
    }

    /// Unix sends SIGTERM; Windows uses channel closure as its exit request.
    pub fn request_exit(&self) -> io::Result<()> {
        self.0.request_exit()
    }

    /// Forcibly terminate this child process.
    pub fn terminate(&self) -> io::Result<()> {
        self.0.terminate()
    }

    /// Wait for OS confirmation and reap the process.
    pub async fn wait(self) -> io::Result<ExitStatus> {
        self.0.wait().await
    }
}

/// The child-side channel and credential, taken once on startup.
#[derive(Debug)]
pub struct Inherited {
    pub channel: SessionChannel,
    pub credential: Credential,
}

/// Start a process with a private session and one-shot credential pipe.
///
/// The credential's core-side copy is zeroized before this call returns.
pub fn spawn(spec: &ChildSpec, credential: Credential) -> io::Result<Spawned> {
    platform::spawn(spec, credential)
}

/// Consume this child's inherited transport and read its credential to EOF.
///
/// Call once during child startup. A second call fails without taking ownership
/// of an already-owned descriptor or handle. The environment contains only
/// descriptor or handle numbers, never credential bytes.
pub fn take_inherited() -> io::Result<Inherited> {
    platform::take_inherited()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_debug_never_discloses_bytes() {
        let credential = Credential::from(b"secret-credential".to_vec());
        assert_eq!(credential.expose(), b"secret-credential");
        assert_eq!(format!("{credential:?}"), "Credential([redacted])");
    }
}
