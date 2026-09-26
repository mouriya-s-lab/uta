//! The OS half of the instance fence (design §7.1 单实例, §7.2 step 1).
//!
//! One exclusive OS file lock per user state root. The lock is held by the
//! open file handle, so it is released when this process ends, including on a
//! crash. Rust opens files non-inheritable (`O_CLOEXEC` / no handle
//! inheritance), so child processes never keep the lock alive after the core
//! has died.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::Path;

/// Holding this value is holding the OS lock. Not `Clone`; released by
/// [`InstanceLock::release`] or when dropped.
#[derive(Debug)]
#[must_use = "dropping the lock releases the fence"]
pub struct InstanceLock {
    file: File,
}

#[derive(Debug, thiserror::Error)]
pub enum FenceError {
    #[error("another UTA core instance holds the lock on this state root")]
    Held,
    #[error("cannot take the instance lock: {0}")]
    Io(#[from] io::Error),
}

pub fn acquire(path: &Path) -> Result<InstanceLock, FenceError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(InstanceLock { file }),
        Err(TryLockError::WouldBlock) => Err(FenceError::Held),
        Err(TryLockError::Error(e)) => Err(FenceError::Io(e)),
    }
}

impl InstanceLock {
    /// Controlled stop step 6: release the fence after the store is closed.
    pub fn release(self) -> io::Result<()> {
        self.file.unlock()
    }
}
