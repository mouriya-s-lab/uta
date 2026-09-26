use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::pin::Pin;
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use zeroize::Zeroizing;

use crate::{ChildProcess, ChildSpec, Credential, Inherited, SessionChannel, Spawned};

const CHANNEL_FD: RawFd = 3;
const CREDENTIAL_FD: RawFd = 4;
const FIRST_SOURCE_FD: RawFd = 5;

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
            let stream = self.stream.take().expect("unregistered channel owns its stream");
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

    fn signal(&self, signal: libc::c_int) -> io::Result<()> {
        // The OS owns the child's PID until it is reaped by wait.
        if unsafe { libc::kill(self.0.id() as libc::pid_t, signal) } == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub(super) fn request_exit(&self) -> io::Result<()> {
        self.signal(libc::SIGTERM)
    }

    pub(super) fn terminate(&self) -> io::Result<()> {
        self.signal(libc::SIGKILL)
    }

    pub(super) async fn wait(mut self) -> io::Result<ExitStatus> {
        tokio::task::spawn_blocking(move || self.0.wait())
            .await
            .map_err(io::Error::other)?
    }
}

fn duplicate_source(fd: RawFd) -> io::Result<OwnedFd> {
    // dup2(…, 3) must not overwrite the source used by dup2(…, 4), or vice versa.
    let duplicated = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, FIRST_SOURCE_FD) };
    if duplicated == -1 {
        return Err(io::Error::last_os_error());
    }
    // fcntl returned a newly owned descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(duplicated) })
}

pub(super) fn spawn(spec: &ChildSpec, credential: Credential) -> io::Result<Spawned> {
    let (parent_stream, child_stream) = UnixStream::pair()?;
    let (credential_read, mut credential_write) = io::pipe()?;
    let child_channel = duplicate_source(child_stream.as_raw_fd())?;
    let child_credential = duplicate_source(credential_read.as_raw_fd())?;
    drop(child_stream);
    drop(credential_read);

    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    if let Some(dir) = &spec.current_dir {
        command.current_dir(dir);
    }
    command.env(crate::SESSION_CHANNEL_ENV, CHANNEL_FD.to_string());
    command.env(crate::CREDENTIAL_ENV, CREDENTIAL_FD.to_string());
    // The duplicated sources are CLOEXEC; dup2 creates the only channel and
    // credential descriptors retained across exec at fd 3 and fd 4.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(child_channel.as_raw_fd(), CHANNEL_FD) == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::dup2(child_credential.as_raw_fd(), CREDENTIAL_FD) == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    // Command's pre_exec closure still owns the parent's copies of the read
    // descriptors. Close those before writing so a failed child cannot keep
    // the pipe artificially readable in the parent.
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

fn check_open(fd: RawFd) -> io::Result<()> {
    if unsafe { libc::fcntl(fd, libc::F_GETFD) } == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
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
    // Reject both malformed/missing variables before taking ownership of any fd.
    expected_fd(channel_env.as_deref(), "3")?;
    expected_fd(credential_env.as_deref(), "4")?;
    check_open(CHANNEL_FD)?;
    check_open(CREDENTIAL_FD)?;
    if INHERITED_CLAIMED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "inherited descriptors already claimed",
        ));
    }

    // Both descriptors have been validated and exclusively claimed. Each is
    // wrapped exactly once; dropping the reader closes fd 4 after EOF.
    let stream = unsafe { UnixStream::from_raw_fd(CHANNEL_FD) };
    let reader = unsafe { std::fs::File::from_raw_fd(CREDENTIAL_FD) };
    let mut bytes = read_credential(reader)?;
    Ok(Inherited {
        channel: SessionChannel(Channel::new(stream)),
        credential: Credential::from(std::mem::take(&mut *bytes)),
    })
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
    fn duplicated_child_source_cannot_collide_with_targets_and_is_cloexec() {
        let (reader, _) = io::pipe().unwrap();
        let source = duplicate_source(reader.as_raw_fd()).unwrap();
        assert!(source.as_raw_fd() >= FIRST_SOURCE_FD);
        let flags = unsafe { libc::fcntl(source.as_raw_fd(), libc::F_GETFD) };
        assert_ne!(flags, -1);
        assert_ne!(flags & libc::FD_CLOEXEC, 0);
    }

    #[test]
    fn credential_read_preserves_bytes_across_buffer_growth() {
        let source: Vec<u8> = (0..10_000u32).map(|index| index as u8).collect();
        assert_eq!(&read_credential(&source[..]).unwrap()[..], source.as_slice());
    }
}
