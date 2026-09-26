//! Windows half: anonymous pipes inherited only by the spawned child through
//! `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`, bridged to async with dedicated threads.

use std::ffi::{OsStr, OsString, c_void};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::os::windows::process::ExitStatusExt;
use std::pin::Pin;
use std::process::ExitStatus;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf};
use tokio::runtime::Runtime;
use windows_sys::Win32::Foundation::{
    DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE, TRUE, WAIT_FAILED, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::IO::CancelSynchronousIo;
use windows_sys::Win32::System::Threading::{
    CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, GetCurrentProcess, GetExitCodeProcess, INFINITE,
    InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROCESS_INFORMATION, STARTUPINFOEXW, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject,
};
use zeroize::Zeroizing;

use crate::{
    CREDENTIAL_ENV, ChildProcess, ChildSpec, Credential, Inherited, SESSION_CHANNEL_ENV,
    SessionChannel, Spawned,
};

/// Capacity of each in-memory duplex and of each pump's transfer buffer.
const BRIDGE_BUFFER: usize = 64 * 1024;

/// Set once the inherited handles have been adopted, so no handle gets two owners.
static INHERITED_TAKEN: AtomicBool = AtomicBool::new(false);

pub(super) fn spawn(spec: &ChildSpec, credential: Credential) -> io::Result<Spawned> {
    let application = wide_nul(spec.program.as_os_str())?;
    let mut command_line = command_line(spec.program.as_os_str(), &spec.args)?;
    let current_dir = spec
        .current_dir
        .as_deref()
        .map(|dir| wide_nul(dir.as_os_str()))
        .transpose()?;

    let (child_read, core_write) = create_pipe()?;
    let (core_read, child_write) = create_pipe()?;
    let (child_credential, core_credential) = create_pipe()?;

    // Only these duplicates are inheritable; the originals close here.
    let child_read = inheritable(child_read)?;
    let child_write = inheritable(child_write)?;
    let child_credential = inheritable(child_credential)?;

    let session_value = format!(
        "{},{}",
        child_read.as_raw_handle() as usize,
        child_write.as_raw_handle() as usize
    );
    let credential_value = (child_credential.as_raw_handle() as usize).to_string();
    let environment = environment_block(std::env::vars_os(), &session_value, &credential_value)?;

    let inherited: [HANDLE; 3] = [
        child_read.as_raw_handle(),
        child_write.as_raw_handle(),
        child_credential.as_raw_handle(),
    ];
    let attributes = HandleListAttribute::new(&inherited)?;

    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.lpAttributeList = attributes.as_ptr();
    let mut info = PROCESS_INFORMATION::default();

    // SAFETY: every pointer refers to a live, NUL-terminated buffer owned by this
    // frame; `inherited` outlives the call as required by the attribute list.
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            command_line.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            TRUE,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            environment.as_ptr().cast::<c_void>(),
            current_dir.as_ref().map_or(ptr::null(), |dir| dir.as_ptr()),
            &startup.StartupInfo,
            &mut info,
        )
    };
    if created == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: CreateProcessW succeeded, so both handles are fresh and owned by us.
    let (process_handle, thread_handle) = unsafe {
        (
            OwnedHandle::from_raw_handle(info.hProcess),
            OwnedHandle::from_raw_handle(info.hThread),
        )
    };
    drop(thread_handle);
    drop(attributes);
    // The child now holds its own copies; closing ours lets EOF and broken-pipe
    // reach each side when the peer goes away.
    drop(child_read);
    drop(child_write);
    drop(child_credential);

    let process = Process {
        handle: process_handle,
        pid: info.dwProcessId,
    };
    let delivered = deliver_credential(core_credential, credential)
        .and_then(|()| Channel::new(File::from(core_read), File::from(core_write)));
    match delivered {
        Ok(channel) => Ok(Spawned {
            channel: SessionChannel(channel),
            process: ChildProcess(process),
        }),
        Err(error) => {
            process.terminate_and_reap();
            Err(error)
        }
    }
}

pub(super) fn take_inherited() -> io::Result<Inherited> {
    let (read, write) = parse_session_channel(&env_var(SESSION_CHANNEL_ENV)?)?;
    let credential = parse_handle(&env_var(CREDENTIAL_ENV)?)?;
    if read == write || read == credential || write == credential {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "inherited handle values must be distinct",
        ));
    }
    if INHERITED_TAKEN.swap(true, Ordering::SeqCst) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "inherited handles were already taken",
        ));
    }
    // SAFETY: the spawning core placed exactly these three inherited handles in
    // our environment; the guard above makes this the only adoption.
    let (read, write, credential) = unsafe {
        (
            File::from_raw_handle(read),
            File::from_raw_handle(write),
            File::from_raw_handle(credential),
        )
    };
    let mut secret = read_credential(credential)?;
    let channel = Channel::new(read, write)?;
    Ok(Inherited {
        channel: SessionChannel(channel),
        credential: Credential::from(std::mem::take(&mut *secret)),
    })
}

/// Writes every credential byte, closes the write end, then zeroizes the copy.
fn deliver_credential(write_end: OwnedHandle, credential: Credential) -> io::Result<()> {
    let mut pipe = File::from(write_end);
    let written = pipe.write_all(credential.expose());
    drop(pipe);
    drop(credential);
    written
}

/// Reads to EOF without leaving unzeroized copies behind on reallocation.
fn read_credential(mut source: impl Read) -> io::Result<Zeroizing<Vec<u8>>> {
    let mut bytes = Zeroizing::new(Vec::new());
    let mut chunk = Zeroizing::new([0u8; 4096]);
    loop {
        let count = match source.read(&mut chunk[..]) {
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

fn env_var(name: &str) -> io::Result<String> {
    std::env::var(name).map_err(|error| match error {
        std::env::VarError::NotPresent => {
            io::Error::new(io::ErrorKind::NotFound, format!("{name} is not set"))
        }
        std::env::VarError::NotUnicode(_) => {
            io::Error::new(io::ErrorKind::InvalidData, format!("{name} is not unicode"))
        }
    })
}

/// Parses `UTA_SESSION_CHANNEL="<read>,<write>"`.
fn parse_session_channel(value: &str) -> io::Result<(RawHandle, RawHandle)> {
    let (read, write) = value.split_once(',').ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{SESSION_CHANNEL_ENV} must be \"<read>,<write>\""),
        )
    })?;
    Ok((parse_handle(read)?, parse_handle(write)?))
}

fn parse_handle(value: &str) -> io::Result<RawHandle> {
    match value.parse::<usize>() {
        Ok(raw) if raw != 0 && raw != usize::MAX => Ok(raw as RawHandle),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid inherited handle value {value:?}"),
        )),
    }
}

/// std's anonymous pipe is `CreatePipe` with null security attributes, so
/// both ends are non-inheritable.
fn create_pipe() -> io::Result<(OwnedHandle, OwnedHandle)> {
    let (read, write) = io::pipe()?;
    Ok((OwnedHandle::from(read), OwnedHandle::from(write)))
}

/// Replaces a non-inheritable handle with an inheritable duplicate.
fn inheritable(handle: OwnedHandle) -> io::Result<OwnedHandle> {
    let mut duplicate: HANDLE = ptr::null_mut();
    // SAFETY: `handle` is a live handle in this process; the out-pointer is valid.
    let duplicated = unsafe {
        let current = GetCurrentProcess();
        DuplicateHandle(
            current,
            handle.as_raw_handle(),
            current,
            &mut duplicate,
            0,
            TRUE,
            DUPLICATE_SAME_ACCESS,
        )
    };
    if duplicated == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: DuplicateHandle succeeded and returned a fresh handle we own.
    Ok(unsafe { OwnedHandle::from_raw_handle(duplicate) })
}

/// A one-entry `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`; the listed handles must
/// outlive it.
struct HandleListAttribute {
    buffer: Vec<usize>,
}

impl HandleListAttribute {
    fn new(handles: &[HANDLE; 3]) -> io::Result<Self> {
        let mut size = 0usize;
        // SAFETY: size query; fails with ERROR_INSUFFICIENT_BUFFER by design.
        unsafe { InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut size) };
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0usize; size.div_ceil(size_of::<usize>())];
        let list: LPPROC_THREAD_ATTRIBUTE_LIST = buffer.as_mut_ptr().cast();
        // SAFETY: `buffer` is at least `size` bytes and pointer-aligned.
        if unsafe { InitializeProcThreadAttributeList(list, 1, 0, &mut size) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let attribute = Self { buffer };
        // SAFETY: the list is initialized; `handles` stays alive until after
        // CreateProcessW in the caller, and the list is dropped afterwards.
        let updated = unsafe {
            UpdateProcThreadAttribute(
                attribute.as_ptr(),
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                handles.as_ptr().cast::<c_void>(),
                size_of_val(handles),
                ptr::null_mut(),
                ptr::null(),
            )
        };
        if updated == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(attribute)
    }

    fn as_ptr(&self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.buffer.as_ptr().cast_mut().cast()
    }
}

impl Drop for HandleListAttribute {
    fn drop(&mut self) {
        // SAFETY: constructed only after successful initialization.
        unsafe { DeleteProcThreadAttributeList(self.as_ptr()) };
    }
}

fn wide_nul(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut wide: Vec<u16> = value.encode_wide().collect();
    if wide.contains(&0) {
        return Err(nul_error());
    }
    wide.push(0);
    Ok(wide)
}

fn nul_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "value contains a NUL character",
    )
}

const QUOTE: u16 = b'"' as u16;
const BACKSLASH: u16 = b'\\' as u16;

/// Builds a NUL-terminated command line that the MSVC runtime (and
/// `CommandLineToArgvW`) splits back into exactly `program` + `args`.
fn command_line(program: &OsStr, args: &[OsString]) -> io::Result<Vec<u16>> {
    let mut line = Vec::new();
    // argv[0] is parsed without escapes, so it is quoted verbatim.
    line.push(QUOTE);
    for unit in program.encode_wide() {
        if unit == 0 || unit == QUOTE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "program path contains a NUL or quote character",
            ));
        }
        line.push(unit);
    }
    line.push(QUOTE);
    for arg in args {
        line.push(u16::from(b' '));
        append_argument(&mut line, arg)?;
    }
    line.push(0);
    Ok(line)
}

fn append_argument(line: &mut Vec<u16>, arg: &OsStr) -> io::Result<()> {
    let units: Vec<u16> = arg.encode_wide().collect();
    if units.contains(&0) {
        return Err(nul_error());
    }
    let needs_quotes = units.is_empty()
        || units
            .iter()
            .any(|&unit| matches!(unit, 0x20 | 0x09 | 0x0A | 0x0B) || unit == QUOTE);
    if !needs_quotes {
        line.extend_from_slice(&units);
        return Ok(());
    }
    line.push(QUOTE);
    let mut backslashes = 0usize;
    for unit in units {
        if unit == BACKSLASH {
            backslashes += 1;
            continue;
        }
        // The Windows argv parser consumes pairs of backslashes before a
        // quote; a literal quote needs one extra backslash.
        line.extend(std::iter::repeat_n(
            BACKSLASH,
            if unit == QUOTE {
                backslashes * 2 + 1
            } else {
                backslashes
            },
        ));
        backslashes = 0;
        line.push(unit);
    }
    // Double the backslashes immediately before the closing quote.
    line.extend(std::iter::repeat_n(BACKSLASH, backslashes * 2));
    line.push(QUOTE);
    Ok(())
}

/// Unicode environment block: the current variables with the two channel
/// variables replaced, sorted case-insensitively, double-NUL terminated.
fn environment_block(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
    session_channel: &str,
    credential: &str,
) -> io::Result<Vec<u16>> {
    let overrides: [(Vec<u16>, Vec<u16>); 2] = [
        (wide(SESSION_CHANNEL_ENV), wide(session_channel)),
        (wide(CREDENTIAL_ENV), wide(credential)),
    ];
    let mut entries: Vec<(Vec<u16>, Vec<u16>)> = vars
        .into_iter()
        .map(|(key, value)| (key.encode_wide().collect(), value.encode_wide().collect()))
        .filter(|(key, _): &(Vec<u16>, Vec<u16>)| {
            !overrides
                .iter()
                .any(|(name, _)| key_order(key).eq(key_order(name)))
        })
        .collect();
    entries.extend(overrides);
    entries.sort_by(|(a, _), (b, _)| key_order(a).cmp(key_order(b)));

    let mut block = Vec::new();
    for (key, value) in entries {
        if key.is_empty() || key.contains(&0) || value.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "environment variable is empty or contains a NUL character",
            ));
        }
        block.extend_from_slice(&key);
        block.push(u16::from(b'='));
        block.extend_from_slice(&value);
        block.push(0);
    }
    block.push(0);
    Ok(block)
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}

fn key_order(key: &[u16]) -> impl Iterator<Item = u16> + '_ {
    key.iter().map(|&unit| match u8::try_from(unit) {
        Ok(byte) => u16::from(byte.to_ascii_uppercase()),
        Err(_) => unit,
    })
}

/// Core or child end of the session channel. Reads come from `inbound`,
/// writes go to `outbound`; each blocking pipe handle is served by its own
/// thread. Field order matters: the duplex ends drop before the pumps join.
#[derive(Debug)]
pub(crate) struct Channel {
    inbound: DuplexStream,
    outbound: DuplexStream,
    _reader: Pump,
    _writer: Pump,
}

impl Channel {
    fn new(read: File, write: File) -> io::Result<Self> {
        let (inbound, inbound_peer) = tokio::io::duplex(BRIDGE_BUFFER);
        let (outbound, outbound_peer) = tokio::io::duplex(BRIDGE_BUFFER);
        let reader = Pump::spawn("uta-channel-read", move |runtime| {
            pump_pipe_to_duplex(runtime, read, inbound_peer)
        })?;
        let writer = match Pump::spawn("uta-channel-write", move |runtime| {
            pump_duplex_to_pipe(runtime, outbound_peer, write)
        }) {
            Ok(writer) => writer,
            Err(error) => {
                drop(inbound);
                drop(reader);
                return Err(error);
            }
        };
        Ok(Self {
            inbound,
            outbound,
            _reader: reader,
            _writer: writer,
        })
    }
}

impl AsyncRead for Channel {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inbound).poll_read(cx, buf)
    }
}

impl AsyncWrite for Channel {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().outbound).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().outbound).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().outbound).poll_shutdown(cx)
    }
}

/// Pipe → duplex. Ends on pipe EOF/error or when the channel's read side is
/// gone; dropping `peer` then delivers EOF (or broken pipe) to the channel.
fn pump_pipe_to_duplex(runtime: &Runtime, mut pipe: File, mut peer: DuplexStream) {
    let mut buffer = vec![0u8; BRIDGE_BUFFER];
    loop {
        let count = match pipe.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(count) => count,
        };
        if runtime.block_on(peer.write_all(&buffer[..count])).is_err() {
            return;
        }
    }
}

/// Duplex → pipe. Ends on channel shutdown/drop or pipe error; dropping `pipe`
/// then delivers EOF to the other process.
fn pump_duplex_to_pipe(runtime: &Runtime, mut peer: DuplexStream, mut pipe: File) {
    let mut buffer = vec![0u8; BRIDGE_BUFFER];
    loop {
        let count = match runtime.block_on(peer.read(&mut buffer)) {
            Ok(0) | Err(_) => return,
            Ok(count) => count,
        };
        if pipe.write_all(&buffer[..count]).is_err() {
            return;
        }
    }
}

/// A bridging thread with its own current-thread runtime. Dropping it cancels
/// any blocked synchronous pipe I/O on that thread and joins it.
#[derive(Debug)]
struct Pump(Option<JoinHandle<()>>);

impl Pump {
    fn spawn(name: &str, body: impl FnOnce(&Runtime) + Send + 'static) -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread().build()?;
        let thread = thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || body(&runtime))?;
        Ok(Self(Some(thread)))
    }
}

impl Drop for Pump {
    fn drop(&mut self) {
        let Some(thread) = self.0.take() else { return };
        // The duplex ends are already closed, so the thread fails its next
        // duplex operation; only a blocking ReadFile/WriteFile can hold it.
        // Cancellation only hits I/O already pending, so repeat until it exits.
        while !thread.is_finished() {
            // SAFETY: the join handle keeps the thread handle open.
            unsafe { CancelSynchronousIo(thread.as_raw_handle()) };
            thread::sleep(Duration::from_millis(1));
        }
        let _ = thread.join();
    }
}

#[derive(Debug)]
pub(crate) struct Process {
    handle: OwnedHandle,
    pid: u32,
}

impl Process {
    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    /// Windows has no exit request for another process; closing the session
    /// channel is the request (design §7.1/§7.2).
    pub(crate) fn request_exit(&self) -> io::Result<()> {
        Ok(())
    }

    pub(crate) fn terminate(&self) -> io::Result<()> {
        // SAFETY: we own a live process handle.
        if unsafe { TerminateProcess(self.handle.as_raw_handle(), 1) } != 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        // Terminating an already-exited process fails; that is still success.
        // SAFETY: we own a live process handle.
        if unsafe { WaitForSingleObject(self.handle.as_raw_handle(), 0) } == WAIT_OBJECT_0 {
            return Ok(());
        }
        Err(error)
    }

    pub(crate) async fn wait(self) -> io::Result<ExitStatus> {
        tokio::task::spawn_blocking(move || self.wait_blocking())
            .await
            .map_err(io::Error::other)?
    }

    fn wait_blocking(&self) -> io::Result<ExitStatus> {
        let handle = self.handle.as_raw_handle();
        // SAFETY: we own a live process handle.
        if unsafe { WaitForSingleObject(handle, INFINITE) } == WAIT_FAILED {
            return Err(io::Error::last_os_error());
        }
        let mut code = 0u32;
        // SAFETY: we own a live process handle; the out-pointer is valid.
        if unsafe { GetExitCodeProcess(handle, &mut code) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(ExitStatus::from_raw(code))
    }

    /// Spawn-failure cleanup: the child must not outlive a failed `spawn`.
    fn terminate_and_reap(self) {
        let _ = self.terminate();
        let _ = self.wait_blocking();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(program: &str, args: &[&str]) -> String {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        let mut wide = command_line(OsStr::new(program), &args).unwrap();
        assert_eq!(wide.pop(), Some(0));
        String::from_utf16(&wide).unwrap()
    }

    #[test]
    fn command_line_round_trips_msvc_quoting() {
        assert_eq!(
            line(r"C:\Program Files\a.exe", &[]),
            r#""C:\Program Files\a.exe""#
        );
        assert_eq!(line("a", &["plain", ""]), r#""a" plain """#);
        assert_eq!(line("a", &["with space"]), r#""a" "with space""#);
        assert_eq!(line("a", &[r"dir\"]), r#""a" dir\"#);
        assert_eq!(line("a", &[r"dir with\"]), r#""a" "dir with\\""#);
        assert_eq!(line("a", &[r#"say "hi""#]), r#""a" "say \"hi\"""#);
        assert_eq!(line("a", &[r#"x\"y"#]), r#""a" "x\\\"y""#);
    }

    #[test]
    fn command_line_rejects_nul_and_quoted_program() {
        assert!(command_line(OsStr::new("a\"b"), &[]).is_err());
        assert!(command_line(OsStr::new("a"), &[OsString::from("x\0y")]).is_err());
    }

    #[test]
    fn environment_block_replaces_channel_vars_case_insensitively() {
        let vars = vec![
            (OsString::from("zeta"), OsString::from("1")),
            (
                OsString::from("uta_session_channel"),
                OsString::from("stale"),
            ),
            (OsString::from("Alpha"), OsString::from("2")),
        ];
        let block = environment_block(vars, "10,11", "12").unwrap();
        assert_eq!(block[block.len() - 2..], [0, 0]);
        let text = String::from_utf16(&block[..block.len() - 2]).unwrap();
        let entries: Vec<&str> = text.split('\0').collect();
        assert_eq!(
            entries,
            [
                "Alpha=2",
                "UTA_CREDENTIAL=12",
                "UTA_SESSION_CHANNEL=10,11",
                "zeta=1"
            ]
        );
    }

    #[test]
    fn session_channel_value_parses_two_handles() {
        let (read, write) = parse_session_channel("40,44").unwrap();
        assert_eq!((read as usize, write as usize), (40, 44));
        assert!(parse_session_channel("40").is_err());
        assert!(parse_session_channel("0,4").is_err());
        assert!(parse_session_channel("4,x").is_err());
    }

    #[test]
    fn credential_read_collects_all_chunks() {
        let secret: Vec<u8> = (0..10_000u32).map(|i| i as u8).collect();
        let read = read_credential(&secret[..]).unwrap();
        assert_eq!(&read[..], &secret[..]);
    }
}
