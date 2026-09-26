use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::pin::Pin;
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use command_fds::{CommandFdExt, FdMapping};
use rustix::process::{Pid, Signal, kill_process};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use zeroize::Zeroizing;

use crate::{ChildProcess, ChildSpec, Credential, Inherited, SessionChannel, Spawned};

const CHANNEL_FD: RawFd = 3;
const CREDENTIAL_FD: RawFd = 4;

// from_raw_fd transfers ownership; only one invocation may claim the inherited descriptors.
static INHERITED_CLAIMED: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub(super) struct Channel {
    stream: Option<UnixStream>,
    registered: Option<tokio::net::UnixStream>,
}

impl Channel {
    fn new(stream: UnixStream) -> Self {
        Self {
            stream: Some(stream),
            registered: None,
        }
    }

    fn registered(&mut self) -> io::Result<&mut tokio::net::UnixStream> {
        if self.registered.is_none() {
            // Spawn and take_inherited are synchronous; registering requires a runtime only
            // when the channel is first polled.
            tokio::runtime::Handle::try_current().map_err(io::Error::other)?;
            self.stream
                .as_ref()
                .expect("unregistered channel owns its stream")
                .set_nonblocking(true)?;
            let stream = self
                .stream
                .take()
                .expect("unregistered channel owns its stream");
            self.registered = Some(tokio::net::UnixStream::from_std(stream)?);
        }
        Ok(self.registered.as_mut().expect("channel is registered"))
    }
}

impl AsyncRead for Channel {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut().registered() {
            Ok(stream) => Pin::new(stream).poll_read(cx, buf),
            Err(error) => Poll::Ready(Err(error)),
        }
    }
}

impl AsyncWrite for Channel {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut().registered() {
            Ok(stream) => Pin::new(stream).poll_write(cx, buf),
            Err(error) => Poll::Ready(Err(error)),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut().registered() {
            Ok(stream) => Pin::new(stream).poll_flush(cx),
            Err(error) => Poll::Ready(Err(error)),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut().registered() {
            Ok(stream) => Pin::new(stream).poll_shutdown(cx),
            Err(error) => Poll::Ready(Err(error)),
        }
    }
}

#[derive(Debug)]
pub(super) struct Process(Child);

impl Process {
    pub(super) fn pid(&self) -> u32 {
        self.0.id()
    }

    fn signal(&self, signal: Signal) -> io::Result<()> {
        // The OS keeps the child's PID reserved until it is reaped by wait.
        kill_process(Pid::from_child(&self.0), signal).map_err(io::Error::from)
    }

    pub(super) fn request_exit(&self) -> io::Result<()> {
        self.signal(Signal::TERM)
    }

    pub(super) fn terminate(&self) -> io::Result<()> {
        self.signal(Signal::KILL)
    }

    pub(super) async fn wait(mut self) -> io::Result<ExitStatus> {
        tokio::task::spawn_blocking(move || self.0.wait())
            .await
            .map_err(io::Error::other)?
    }
}

pub(super) fn spawn(spec: &ChildSpec, credential: Credential) -> io::Result<Spawned> {
    let (parent_stream, child_stream) = UnixStream::pair()?;
    let (credential_read, mut credential_write) = io::pipe()?;

    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    if let Some(dir) = &spec.current_dir {
        command.current_dir(dir);
    }
    command.env(crate::SESSION_CHANNEL_ENV, CHANNEL_FD.to_string());
    command.env(crate::CREDENTIAL_ENV, CREDENTIAL_FD.to_string());
    // Both sources are CLOEXEC. The mappings are given in one call so the
    // library can move a source that sits on the other target out of the way;
    // fd 3 and fd 4 are then the only channel and credential descriptors that
    // survive exec.
    command
        .fd_mappings(vec![
            FdMapping {
                parent_fd: OwnedFd::from(child_stream),
                child_fd: CHANNEL_FD,
            },
            FdMapping {
                parent_fd: OwnedFd::from(credential_read),
                child_fd: CREDENTIAL_FD,
            },
        ])
        .map_err(io::Error::other)?;
    let mut child = command.spawn()?;
    // The Command still owns the parent's copies of the mapped descriptors.
    // Close those before writing so a failed child cannot keep the pipe
    // artificially readable in the parent.
    drop(command);

    let result = credential_write.write_all(credential.expose());
    drop(credential_write);
    drop(credential);
    if let Err(error) = result {
        // No child process handle can be returned after failed delivery.
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }

    Ok(Spawned {
        channel: SessionChannel(Channel::new(parent_stream)),
        process: ChildProcess(Process(child)),
    })
}

fn expected_fd(value: Option<&OsStr>, expected: &str) -> io::Result<()> {
    if value == Some(OsStr::new(expected)) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("expected inherited descriptor {expected}"),
        ))
    }
}

fn read_credential(mut reader: impl Read) -> io::Result<Zeroizing<Vec<u8>>> {
    let mut bytes = Zeroizing::new(Vec::new());
    let mut chunk = Zeroizing::new([0u8; 4096]);
    loop {
        let count = match reader.read(&mut chunk[..]) {
            Ok(0) => return Ok(bytes),
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if bytes.capacity() - bytes.len() < count {
            let capacity = (bytes.len() + count).max(bytes.capacity() * 2);
            let mut grown = Zeroizing::new(Vec::with_capacity(capacity));
            grown.extend_from_slice(&bytes);
            bytes = grown;
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
}

pub(super) fn take_inherited() -> io::Result<Inherited> {
    let channel_env = std::env::var_os(crate::SESSION_CHANNEL_ENV);
    let credential_env = std::env::var_os(crate::CREDENTIAL_ENV);
    // Reject both malformed/missing variables before claiming any fd.
    expected_fd(channel_env.as_deref(), "3")?;
    expected_fd(credential_env.as_deref(), "4")?;
    if INHERITED_CLAIMED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "inherited descriptors already claimed",
        ));
    }
    let (stream, reader) = adopt_inherited()?;
    let mut bytes = read_credential(reader)?;
    Ok(Inherited {
        channel: SessionChannel(Channel::new(stream)),
        credential: Credential::from(std::mem::take(&mut *bytes)),
    })
}

/// The only place a raw descriptor number is asserted to be owned. Called
/// once, after the claim; a failure leaves the descriptors unclaimable.
fn adopt_inherited() -> io::Result<(UnixStream, File)> {
    // SAFETY: the claim guarantees this runs at most once per process, so
    // nothing else in the process owns fd 3 or fd 4. F_GETFD on a raw number
    // takes no ownership; both descriptors are proven open before either is
    // wrapped, and each is wrapped exactly once. Dropping the reader closes
    // fd 4 after EOF.
    unsafe {
        for fd in [CHANNEL_FD, CREDENTIAL_FD] {
            if libc::fcntl(fd, libc::F_GETFD) == -1 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok((
            UnixStream::from_raw_fd(CHANNEL_FD),
            File::from_raw_fd(CREDENTIAL_FD),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_descriptors_are_exactly_reserved_fds() {
        assert!(expected_fd(Some(OsStr::new("3")), "3").is_ok());
        assert!(expected_fd(Some(OsStr::new("04")), "4").is_err());
        assert!(expected_fd(Some(OsStr::new("5")), "4").is_err());
        assert!(expected_fd(None, "4").is_err());
    }

    #[test]
    fn credential_read_preserves_bytes_across_buffer_growth() {
        let source: Vec<u8> = (0..10_000u32).map(|index| index as u8).collect();
        assert_eq!(
            &read_credential(&source[..]).unwrap()[..],
            source.as_slice()
        );
    }
}
