//! OS process identity and exit confirmation.
//!
//! The process table (design §7.5) is an index into OS processes, keyed by
//! `(pid, start_time)`; whether a process is alive is asked of the OS only.
//! [`ExitConfirmed`] is the only proof the core accepts that an indexed process
//! has ended: it can be obtained only from this crate, and only after the OS
//! reported that no process with that `(pid, start_time)` exists.

use std::io;
use std::time::Duration;

use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System};

#[cfg(unix)]
use sysinfo::Signal;

/// Identity of one OS process: the pid together with the OS-reported start
/// time, so that a reused pid is not mistaken for the same process.
///
/// This is an index, not a capability: constructing one from stored numbers
/// grants nothing. Proof of exit is [`ExitConfirmed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OsProcessId {
    pid: u32,
    /// Start time as reported by the OS, in seconds since the Unix epoch.
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

fn refreshed_process(pid: u32) -> (System, Pid) {
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(std::slice::from_ref(&pid)),
        true,
        ProcessRefreshKind::nothing(),
    );
    (system, pid)
}

fn process_is_live(process: &Process) -> bool {
    // A zombie still has a process-table entry, but has already exited and
    // cannot receive a signal. Treat it as exited while its parent reaps it.
    process.status() != ProcessStatus::Zombie
}

fn matching_process(system: &System, id: OsProcessId) -> Option<&Process> {
    system
        .process(Pid::from_u32(id.pid))
        .filter(|process| process.start_time() == id.start_time && process_is_live(process))
}

/// Returns the identity of a live process as reported by the OS.
pub fn observe(pid: u32) -> Option<OsProcessId> {
    let (system, process_pid) = refreshed_process(pid);
    let process = system.process(process_pid)?;
    process_is_live(process).then_some(OsProcessId::from_row(pid, process.start_time()))
}

/// Returns whether the OS currently reports the same live `(pid, start_time)`.
pub fn is_running(id: OsProcessId) -> bool {
    let (system, _) = refreshed_process(id.pid);
    matching_process(&system, id).is_some()
}

#[cfg(unix)]
fn signal_if_running(id: OsProcessId, signal: Signal) -> io::Result<()> {
    let (system, _) = refreshed_process(id.pid);
    let Some(process) = matching_process(&system, id) else {
        return Ok(());
    };

    match process.kill_with(signal) {
        Some(true) => Ok(()),
        Some(false) => Err(io::Error::last_os_error()),
        None => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the requested process signal is unsupported",
        )),
    }
}

/// Requests graceful exit from the identified process.
///
/// On Unix, this sends SIGTERM only while the OS reports the same live process
/// identity. On Windows, arbitrary processes have no graceful termination
/// signal, so this is a no-op: the caller requests exit by closing the process
/// channel.
#[cfg(unix)]
pub fn request_exit(id: OsProcessId) -> io::Result<()> {
    signal_if_running(id, Signal::Term)
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
    signal_if_running(id, Signal::Kill)
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
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        TerminateProcess,
    };

    if !is_running(id) {
        return Ok(());
    }

    // Open a handle and validate its creation time before terminating it. This
    // prevents a reused PID from turning a stale identity into a kill request.
    // SAFETY: OpenProcess takes only scalar arguments and returns an owned handle.
    let raw_handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
            0,
            id.pid,
        )
    };

    if raw_handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    let handle = ProcessHandle(raw_handle);

    let mut creation_time = FILETIME::default();
    let mut exit_time = FILETIME::default();
    let mut kernel_time = FILETIME::default();
    let mut user_time = FILETIME::default();
    // SAFETY: The handle is open, and each output pointer refers to a local FILETIME.
    if unsafe {
        GetProcessTimes(
            handle.0,
            &mut creation_time,
            &mut exit_time,
            &mut kernel_time,
            &mut user_time,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }

    const WINDOWS_EPOCH_TICKS: u64 = 116_444_736_000_000_000;
    let creation_ticks =
        (u64::from(creation_time.dwHighDateTime) << 32) | u64::from(creation_time.dwLowDateTime);
    let Some(start_time) = creation_ticks
        .checked_sub(WINDOWS_EPOCH_TICKS)
        .map(|ticks| ticks / 10_000_000)
    else {
        return Ok(());
    };
    if start_time != id.start_time {
        return Ok(());
    }

    // SAFETY: The validated process handle is open with PROCESS_TERMINATE access.
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

/// Waits until the OS no longer reports this process as running.
///
/// The OS is the confirmer, so this has no overall timeout.
pub async fn confirm_exit(id: OsProcessId) -> ExitConfirmed {
    while is_running(id) {
        tokio::time::sleep(EXIT_POLL_INTERVAL).await;
    }
    ExitConfirmed { id }
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
    fn terminate_ends_matching_process() {
        let mut child = RunningChild::spawn();
        let id = observe(child.0.id()).expect("observe spawned process");

        terminate(id).expect("terminate spawned process");
        child.0.wait().expect("reap terminated process");
        assert!(!is_running(id));
    }
}
