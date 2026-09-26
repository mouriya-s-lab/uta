//! The UTA core daemon.
//!
//! Startup (design §7.2): take the fence (OS lock, then the instance row on
//! the core thread), reclaim orphans of earlier instances, then run until the
//! OS asks to stop. Controlled stop ends inner lifecycles first, writes the
//! instance end anchor, closes SQLite on the core thread, joins it, and only
//! then releases the OS lock.

mod core_thread;
mod exit;
mod fence;
mod state_root;

use std::process::ExitCode;
use std::time::Duration;

use tracing::{error, info, warn};
use uta_store::{InstanceEnd, OpenError};

use crate::core_thread::{CoreHandle, StartError, StopError};
use crate::exit::Exit;
use crate::fence::FenceError;

/// How long an orphan gets between the exit request and forced termination.
const ORPHAN_GRACE: Duration = Duration::from_secs(5);
/// Pause before retrying an orphan the OS refused to terminate.
const ORPHAN_RETRY: Duration = Duration::from_secs(1);

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("UTA_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    run().code()
}

/// How the running phase ended, before the controlled stop.
#[derive(Debug)]
enum Running {
    /// The OS asked to stop.
    StopRequested,
    /// The instance cannot run safely; stop it and report failure.
    Failed,
}

fn run() -> Exit {
    let dir = match state_root::core_dir() {
        Ok(dir) => dir,
        Err(e) => {
            error!("cannot prepare the core state directory: {e}");
            return Exit::Failure;
        }
    };

    // Built before the fence: nothing can fail between taking the fence and
    // entering the running phase except the phase itself.
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            error!("cannot start the async runtime: {e}");
            return Exit::Failure;
        }
    };

    // Declared before `core`, so on any early return `core` is dropped (its
    // thread joined) before the lock is released.
    let lock = match fence::acquire(&dir.join("uta.lock")) {
        Ok(lock) => lock,
        Err(FenceError::Held) => {
            error!("{}", FenceError::Held);
            return Exit::FenceHeld;
        }
        Err(e) => {
            error!("{e}");
            return Exit::Failure;
        }
    };

    let (core, ready) = match core_thread::start(dir.join("uta.db")) {
        Ok(started) => started,
        Err(e) => {
            error!("{e}");
            return match e {
                StartError::Open(
                    OpenError::FormatTooNew { .. }
                    | OpenError::FormatUnreadable(_)
                    | OpenError::Migration { .. },
                ) => Exit::FormatIncompatible,
                _ => Exit::Failure,
            };
        }
    };
    match ready.previous {
        None => info!(instance = %ready.instance, "fence taken; first instance on this state root"),
        Some(prev) => match prev.end {
            InstanceEnd::Stopped { .. } => {
                info!(instance = %ready.instance, previous = %prev.id, "fence taken; previous instance stopped")
            }
            InstanceEnd::NoEndAnchor => {
                warn!(instance = %ready.instance, previous = %prev.id, "fence taken; previous instance has no end anchor (crashed)")
            }
        },
    }

    let running = runtime.block_on(running_phase(&core));
    drop(runtime);

    info!("controlled stop");
    match core.stop() {
        Ok(()) => {}
        Err(StopError::CloseAfterAnchor(e)) => {
            error!("end anchor written, but closing the store failed: {e}");
            return release(lock, Exit::Failure);
        }
        Err(e) => {
            error!("controlled stop failed: {e}");
            return Exit::StopFailed;
        }
    }
    release(
        lock,
        match running {
            Running::StopRequested => Exit::Stopped,
            Running::Failed => Exit::Failure,
        },
    )
}

/// Controlled stop step 6.
fn release(lock: fence::InstanceLock, exit: Exit) -> Exit {
    match lock.release() {
        Ok(()) => {
            info!("instance ended; fence released");
            exit
        }
        Err(e) => {
            error!("end anchor written but releasing the lock failed: {e}");
            Exit::Failure
        }
    }
}

/// Everything between taking the fence and the stop request.
async fn running_phase(core: &CoreHandle) -> Running {
    // Listeners are installed before anything waits: a stop request during
    // orphan reclamation or after "core ready" must reach the controlled
    // stop, not the OS default action.
    let mut signals = match StopSignals::install() {
        Ok(signals) => signals,
        Err(e) => {
            error!("cannot listen for stop signals ({e}); stopping");
            return Running::Failed;
        }
    };
    match reclaim_orphans(core, &mut signals).await {
        Reclaim::Done => {}
        Reclaim::StopRequested => return Running::StopRequested,
        Reclaim::Failed => return Running::Failed,
    }
    info!("core ready");
    signals.recv().await;
    Running::StopRequested
}

#[derive(Debug)]
enum Reclaim {
    Done,
    StopRequested,
    Failed,
}

/// Design §7.2 step 1 "回收孤儿": every process-table row of an earlier
/// instance whose `(pid, start_time)` still matches is asked to exit, forced
/// after the grace period, and its row cleared once the OS confirms the exit.
/// The instance does not become ready until every row is cleared; a stop
/// request interrupts the wait (the rows stay for the next instance).
async fn reclaim_orphans(core: &CoreHandle, signals: &mut StopSignals) -> Reclaim {
    let rows = match core.other_instance_processes().await {
        Ok(rows) => rows,
        Err(e) => {
            error!("cannot read the process table, orphans cannot be reclaimed: {e}");
            return Reclaim::Failed;
        }
    };
    for row in rows {
        let exited = loop {
            tokio::select! {
                result = uta_proc::reclaim(row.process, ORPHAN_GRACE) => match result {
                    Ok(exited) => break exited,
                    Err(e) => warn!(pid = row.process.pid(), "orphan not yet terminated: {e}"),
                },
                _ = signals.recv() => return Reclaim::StopRequested,
            }
            tokio::select! {
                _ = tokio::time::sleep(ORPHAN_RETRY) => {}
                _ = signals.recv() => return Reclaim::StopRequested,
            }
        };
        match core.clear_process(exited).await {
            Ok(()) => info!(pid = row.process.pid(), instance = %row.instance, "orphan reclaimed"),
            Err(e) => {
                error!(
                    pid = row.process.pid(),
                    "clearing the process row failed: {e}"
                );
                return Reclaim::Failed;
            }
        }
    }
    Reclaim::Done
}

/// The OS stop requests the core honours: on Unix the terminal interrupt and
/// the service manager's SIGTERM; on Windows the console interrupts and the
/// close / shutdown events. Registered when constructed.
#[derive(Debug)]
struct StopSignals {
    #[cfg(unix)]
    interrupt: tokio::signal::unix::Signal,
    #[cfg(unix)]
    terminate: tokio::signal::unix::Signal,
    #[cfg(windows)]
    ctrl_c: tokio::signal::windows::CtrlC,
    #[cfg(windows)]
    ctrl_break: tokio::signal::windows::CtrlBreak,
    #[cfg(windows)]
    ctrl_close: tokio::signal::windows::CtrlClose,
    #[cfg(windows)]
    ctrl_shutdown: tokio::signal::windows::CtrlShutdown,
}

impl StopSignals {
    #[cfg(unix)]
    fn install() -> std::io::Result<Self> {
        use tokio::signal::unix::{SignalKind, signal};
        Ok(Self {
            interrupt: signal(SignalKind::interrupt())?,
            terminate: signal(SignalKind::terminate())?,
        })
    }

    #[cfg(windows)]
    fn install() -> std::io::Result<Self> {
        use tokio::signal::windows;
        Ok(Self {
            ctrl_c: windows::ctrl_c()?,
            ctrl_break: windows::ctrl_break()?,
            ctrl_close: windows::ctrl_close()?,
            ctrl_shutdown: windows::ctrl_shutdown()?,
        })
    }

    /// Resolves at the first stop request.
    async fn recv(&mut self) {
        #[cfg(unix)]
        tokio::select! {
            _ = self.interrupt.recv() => {}
            _ = self.terminate.recv() => {}
        }
        #[cfg(windows)]
        tokio::select! {
            _ = self.ctrl_c.recv() => {}
            _ = self.ctrl_break.recv() => {}
            _ = self.ctrl_close.recv() => {}
            _ = self.ctrl_shutdown.recv() => {}
        }
    }
}
