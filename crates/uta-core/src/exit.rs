//! Process exit codes of the core daemon. Design core/core-process/design.md §4.7 requires dedicated codes
//! for core-level refusals to start; their values are fixed here and listed in
//! ARCHITECTURE.md.

use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Controlled stop completed: end anchor written, lock released.
    Stopped,
    /// Any failure without a dedicated code (I/O on the state directory,
    /// SQLite errors other than format), reported on stderr.
    Failure,
    /// Step 1: another instance holds the fence on this state root.
    FenceHeld,
    /// Step 2: the file's format version is newer than this build, or the
    /// forward migration failed (the file stays at its old version).
    FormatIncompatible,
    /// Controlled stop could not complete; no end anchor, fence released only
    /// by process exit (design core/core-process/design.md §4.7.4 "停止失败").
    StopFailed,
}

impl Exit {
    pub fn code(self) -> ExitCode {
        ExitCode::from(match self {
            Exit::Stopped => 0,
            Exit::Failure => 1,
            Exit::FenceHeld => 10,
            Exit::FormatIncompatible => 11,
            Exit::StopFailed => 14,
        })
    }
}
