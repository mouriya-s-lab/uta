//! OS process identity and exit confirmation.
//!
//! The process table (design §7.5) is an index into OS processes, keyed by
//! `(pid, start_time)`; whether a process is alive is asked of the OS only.
//! [`ExitConfirmed`] is the only proof the core accepts that an indexed process
//! has ended: it can be obtained only from this crate, and only after the OS
//! reported that no process with that `(pid, start_time)` exists.

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
