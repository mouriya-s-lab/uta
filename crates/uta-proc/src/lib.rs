//! OS process identity and exit confirmation.
//!
//! The process table (design §7.5) is an index into OS processes, keyed by
//! `(pid, start_time)`; whether a process is alive is asked of the OS only.
//! [`ExitConfirmed`] is the only proof the core accepts that an indexed process
//! has ended: it can be obtained only from this crate, and only after the OS
//! reported that no process with that `(pid, start_time)` exists.

use std::io;
use std::time::Duration;

/// Identity of one OS process: the pid together with an opaque, platform-precise
/// OS start token. Persist the token unchanged and compare it only for equality;
/// its units differ between Linux, macOS and Windows.
///
/// This is an index, not a capability: constructing one from stored numbers
/// grants nothing. Proof of exit is [`ExitConfirmed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OsProcessId {
    pid: u32,
    /// Opaque OS start token; not a Unix timestamp.
    start_time: u64,
}

impl OsProcessId {
    /// Rebuilds an identity from a process-table row.
    pub fn from_row(pid: u32, start_time: u64) -> Self {
        Self { pid, start_time }
    }

    pub fn pid(self) -> u32 {
        self.pid
    }

    pub fn start_time(self) -> u64 {
        self.start_time
    }
}

/// Proof that the OS no longer has the process identified by [`Self::id`].
///
/// Not `Clone`, not `Copy`, not constructible outside this crate: clearing a
/// process-table row consumes it, so a row is cleared at most once and never
/// without the OS having confirmed the exit.
#[derive(Debug)]
#[must_use = "an exit confirmation exists to clear the process-table row"]
pub struct ExitConfirmed {
    id: OsProcessId,
}

impl ExitConfirmed {
    pub fn id(&self) -> OsProcessId {
        self.id
    }
}

const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// An OS query that could not establish liveness must never prove an exit.
#[derive(Debug)]
enum Liveness {
    Alive,
    Gone,
    Unknown(io::Error),
}

fn token_liveness(id: OsProcessId, token: io::Result<Option<u64>>) -> Liveness {
    match token {
        Ok(Some(token)) if token == id.start_time => Liveness::Alive,
        Ok(_) => Liveness::Gone,
        Err(error) => Liveness::Unknown(error),
    }
}

fn liveness(id: OsProcessId) -> Liveness {
    token_liveness(id, live_start_token(id.pid))
}

/// Reads a single /proc stat record. `comm` can contain both whitespace and ')',
/// so fields must be counted only after its final closing parenthesis.
#[cfg(target_os = "linux")]
fn parse_linux_stat(stat: &str) -> io::Result<Option<u64>> {
    let malformed = || io::Error::new(io::ErrorKind::InvalidData, "malformed /proc stat");
    let (_, fields) = stat.rsplit_once(')').ok_or_else(malformed)?;
    let mut fields = fields.split_whitespace();
    let state = fields.next().ok_or_else(malformed)?;
    let token = fields.nth(18).ok_or_else(malformed)?; // field 22, after state (field 3)
    let start_time = token.parse::<u64>().map_err(|_| malformed())?;
    match state {
        "Z" | "X" | "x" => Ok(None),
        "R" | "S" | "D" | "T" | "t" | "W" | "I" | "P" => Ok(Some(start_time)),
        _ => Err(malformed()),
    }
}

#[cfg(target_os = "linux")]
fn live_start_token(pid: u32) -> io::Result<Option<u64>> {
    let stat = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    parse_linux_stat(&stat)
}

#[cfg(target_os = "macos")]
fn live_start_token(pid: u32) -> io::Result<Option<u64>> {
    use std::mem::{MaybeUninit, size_of};

    let pid = i32::try_from(pid).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "pid is outside the OS pid range",
        )
    })?;
    let mut info = MaybeUninit::<libc::proc_bsdinfo>::uninit();
    // SAFETY: proc_pidinfo writes at most the supplied size to our local buffer.
    // Reset errno so a zero-byte return cannot inherit a previous ESRCH.
    let count = unsafe {
        *libc::__error() = 0;
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size_of::<libc::proc_bsdinfo>() as libc::c_int,
        )
    };
    if count != size_of::<libc::proc_bsdinfo>() as libc::c_int {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(None)
        } else if count > 0 {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "short proc_pidinfo result",
            ))
        } else {
            Err(error)
        };
    }
    // SAFETY: the OS filled the complete proc_bsdinfo structure.
    let info = unsafe { info.assume_init() };
    if info.pbi_pid != pid as u32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "proc_pidinfo returned a different pid",
        ));
    }
    if info.pbi_status == libc::SZOMB {
        return Ok(None);
    }
    let start_time = info
        .pbi_start_tvsec
        .checked_mul(1_000_000)
        .and_then(|time| time.checked_add(info.pbi_start_tvusec))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "process start time overflow"))?;
    Ok(Some(start_time))
}

#[cfg(windows)]
fn open_process(pid: u32, access: u32) -> io::Result<Option<ProcessHandle>> {
    use windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER;
    use windows_sys::Win32::System::Threading::OpenProcess;

    // SAFETY: OpenProcess accepts scalar parameters and returns an owned handle.
    let raw = unsafe { OpenProcess(access, 0, pid) };
    if raw.is_null() {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
            Ok(None)
        } else {
            Err(error)
        };
    }
    Ok(Some(ProcessHandle(raw)))
}

#[cfg(windows)]
fn handle_start_token(handle: &ProcessHandle) -> io::Result<Option<u64>> {
    use windows_sys::Win32::Foundation::{FILETIME, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Threading::{GetProcessTimes, WaitForSingleObject};

    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: This owned handle is open; each output points to a valid FILETIME.
    if unsafe { GetProcessTimes(handle.0, &mut creation, &mut exit, &mut kernel, &mut user) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: This owned handle has PROCESS_SYNCHRONIZE access.
    match unsafe { WaitForSingleObject(handle.0, 0) } {
        WAIT_OBJECT_0 => Ok(None),
        WAIT_TIMEOUT => Ok(Some(
            (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime),
        )),
        _ => Err(io::Error::last_os_error()),
    }
}

#[cfg(windows)]
fn live_start_token(pid: u32) -> io::Result<Option<u64>> {
    use windows_sys::Win32::System::Threading::{
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    };
    let Some(handle) = open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE)?
    else {
        return Ok(None);
    };
    handle_start_token(&handle)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn live_start_token(_pid: u32) -> io::Result<Option<u64>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "process inspection is unsupported on this platform",
    ))
}

/// Returns the identity of a live process as reported by the OS. A failed
/// query cannot provide an identity; it must not be interpreted as an exit.
pub fn observe(pid: u32) -> Option<OsProcessId> {
    live_start_token(pid)
        .ok()
        .flatten()
        .map(|token| OsProcessId::from_row(pid, token))
}

/// Returns false only if the OS confirmed this indexed identity is gone.
pub fn is_running(id: OsProcessId) -> bool {
    !matches!(liveness(id), Liveness::Gone)
}

#[cfg(unix)]
fn signal_if_running(id: OsProcessId, signal: libc::c_int) -> io::Result<()> {
    match liveness(id) {
        Liveness::Gone => return Ok(()),
        Liveness::Unknown(error) => return Err(error),
        Liveness::Alive => {}
    }
    let pid = libc::pid_t::try_from(id.pid).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "pid is outside the OS pid range",
        )
    })?;
    // Unix kill is addressed by PID; a PID reuse between the probe and this
    // syscall cannot be ruled out without a handle-based OS signaling API.
    // SAFETY: Only the matching, currently live positive PID is signaled.
    if unsafe { libc::kill(pid, signal) } == -1 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error);
        }
    }
    Ok(())
}

/// Requests graceful exit from the identified process.
///
/// On Unix, this sends SIGTERM only while the OS reports the same live process
/// identity. On Windows, arbitrary processes have no graceful termination
/// signal, so this is a no-op: the caller requests exit by closing the process
/// channel.
#[cfg(unix)]
pub fn request_exit(id: OsProcessId) -> io::Result<()> {
    signal_if_running(id, libc::SIGTERM)
}

/// On Windows, arbitrary processes have no graceful termination signal. The
/// exit request is the caller closing the process channel, so this is a no-op.
#[cfg(windows)]
pub fn request_exit(_id: OsProcessId) -> io::Result<()> {
    Ok(())
}

#[cfg(not(any(unix, windows)))]
pub fn request_exit(_id: OsProcessId) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "graceful process exit is unsupported on this platform",
    ))
}

/// Immediately terminates the identified process if it is still running.
#[cfg(unix)]
pub fn terminate(id: OsProcessId) -> io::Result<()> {
    signal_if_running(id, libc::SIGKILL)
}

/// Immediately terminates the identified process if it is still running.
#[cfg(windows)]
pub fn terminate(id: OsProcessId) -> io::Result<()> {
    terminate_windows(id)
}

#[cfg(not(any(unix, windows)))]
pub fn terminate(_id: OsProcessId) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "process termination is unsupported on this platform",
    ))
}

#[cfg(windows)]
fn terminate_windows(id: OsProcessId) -> io::Result<()> {
    use windows_sys::Win32::System::Threading::{
        PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, TerminateProcess,
    };

    // Keep this handle from identity validation through termination: a PID may
    // be reused between two separate OpenProcess calls.
    let Some(handle) = open_process(
        id.pid,
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_TERMINATE,
    )?
    else {
        return Ok(());
    };
    match token_liveness(id, handle_start_token(&handle)) {
        Liveness::Alive => {}
        Liveness::Gone => return Ok(()),
        Liveness::Unknown(error) => return Err(error),
    }
    // SAFETY: The open handle is the matching process and has PROCESS_TERMINATE access.
    if unsafe { TerminateProcess(handle.0, 1) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(windows)]
struct ProcessHandle(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // SAFETY: This wrapper owns the handle returned by OpenProcess.
        let _ = unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
    }
}

/// Waits until the OS positively reports this indexed process has exited.
///
/// Unknown OS query outcomes keep polling; there is no overall timeout.
pub async fn confirm_exit(id: OsProcessId) -> ExitConfirmed {
    loop {
        if matches!(liveness(id), Liveness::Gone) {
            return ExitConfirmed { id };
        }
        tokio::time::sleep(EXIT_POLL_INTERVAL).await;
    }
}

/// Requests graceful exit, waits up to `grace`, then forcibly terminates and
/// confirms that the identified process has exited.
pub async fn reclaim(id: OsProcessId, grace: Duration) -> io::Result<ExitConfirmed> {
    request_exit(id)?;
    match tokio::time::timeout(grace, confirm_exit(id)).await {
        Ok(confirmed) => Ok(confirmed),
        Err(_) => {
            terminate(id)?;
            Ok(confirm_exit(id).await)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Command, Stdio};
    use tokio::runtime::Builder;

    struct RunningChild(Child);

    impl RunningChild {
        fn spawn() -> Self {
            #[cfg(unix)]
            let child = Command::new("sleep")
                .arg("30")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn sleep");

            #[cfg(windows)]
            let child = Command::new("ping")
                .args(["-n", "30", "127.0.0.1"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn ping");

            Self(child)
        }
    }

    impl Drop for RunningChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("create Tokio runtime")
    }

    #[test]
    fn observe_returns_live_process_identity() {
        let mut child = RunningChild::spawn();
        let id = observe(child.0.id()).expect("observe spawned process");
        assert_eq!(id.pid(), child.0.id());
        assert_eq!(observe(child.0.id()), Some(id));
        assert!(is_running(id));

        child.0.kill().expect("kill spawned process");
        child.0.wait().expect("reap spawned process");
        assert!(!is_running(id));
    }

    #[test]
    fn stale_start_time_neither_matches_nor_terminates_live_process() {
        let child = RunningChild::spawn();
        let id = observe(child.0.id()).expect("observe spawned process");
        let stale_id = OsProcessId::from_row(id.pid(), id.start_time().wrapping_add(1));

        assert!(!is_running(stale_id));
        request_exit(stale_id).expect("request exit for stale identity");
        terminate(stale_id).expect("terminate stale identity");
        assert!(is_running(id));
        assert_eq!(observe(child.0.id()), Some(id));
    }

    #[test]
    fn reclaim_confirms_exit_of_spawned_process() {
        let mut child = RunningChild::spawn();
        let id = observe(child.0.id()).expect("observe spawned process");
        let confirmed = runtime()
            .block_on(reclaim(id, Duration::from_millis(100)))
            .expect("reclaim spawned process");

        assert_eq!(confirmed.id(), id);
        assert!(!is_running(id));
        child.0.wait().expect("reap reclaimed process");
    }

    #[test]
    fn confirmation_waits_for_actual_exit() {
        let mut child = RunningChild::spawn();
        let id = observe(child.0.id()).expect("observe spawned process");
        let runtime = runtime();
        assert!(
            runtime
                .block_on(async {
                    tokio::time::timeout(Duration::from_millis(100), confirm_exit(id)).await
                })
                .is_err(),
            "live process cannot have exit confirmation"
        );
        assert!(is_running(id));
        child.0.kill().expect("kill spawned process");
        child.0.wait().expect("reap spawned process");
        assert_eq!(runtime.block_on(confirm_exit(id)).id(), id);
    }

    #[test]
    fn terminate_ends_matching_process() {
        let mut child = RunningChild::spawn();
        let id = observe(child.0.id()).expect("observe spawned process");

        terminate(id).expect("terminate spawned process");
        child.0.wait().expect("reap terminated process");
        assert!(!is_running(id));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_stat_parses_start_tick_after_complex_process_name() {
        let stat = format!("123 (name with ) spaces) S {} 987654 0", "0 ".repeat(18));
        assert_eq!(parse_linux_stat(&stat).expect("parse stat"), Some(987654));
        let zombie = stat.replacen(") S ", ") Z ", 1);
        assert_eq!(parse_linux_stat(&zombie).expect("parse zombie stat"), None);
        assert!(parse_linux_stat("123 (truncated) S 0").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn process_exited_with_code_259_is_gone() {
        // The command waits long enough to observe it before exiting. The
        // Child retains a process handle even after wait, exposing the old
        // GetExitCodeProcess/STILL_ACTIVE ambiguity.
        let mut child = Command::new("cmd")
            .args(["/C", "ping -n 3 127.0.0.1 >NUL & exit /B 259"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn cmd");
        let id = observe(child.id()).expect("observe running cmd");
        assert!(is_running(id));
        assert_eq!(child.wait().expect("wait cmd").code(), Some(259));
        assert!(!is_running(id));
        assert_eq!(runtime().block_on(confirm_exit(id)).id(), id);
    }
}
