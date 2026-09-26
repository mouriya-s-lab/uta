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
use uta_store::InstanceEnd;

use crate::core_thread::{CoreHandle, StartError};
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

fn run() -> Exit {
    let dir = match state_root::core_dir() {
        Ok(dir) => dir,
        Err(e) => {
            error!("cannot prepare the core state directory: {e}");
            return Exit::Failure;
        }
    };

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
                    uta_store::OpenError::FormatTooNew { .. }
                    | uta_store::OpenError::Migration { .. },
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

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            error!("cannot start the async runtime: {e}");
            // No inner work has started; the instance ends without an anchor,
            // like a crash, and the successor's fence takes over.
            return Exit::Failure;
        }
    };
    let stopped = runtime.block_on(async {
        reclaim_orphans(&core).await;
        info!("core ready");
        stop_requested().await;
        info!("controlled stop");
        core.stop().await
    });
    drop(runtime);

    match stopped {
        Ok(()) => match lock.release() {
            Ok(()) => {
                info!("instance ended; fence released");
                Exit::Stopped
            }
            Err(e) => {
                error!("end anchor written but releasing the lock failed: {e}");
                Exit::Failure
            }
        },
        Err(e) => {
            error!("controlled stop failed: {e}");
            Exit::StopFailed
        }
    }
}

/// Design §7.2 step 1 "回收孤儿": every process-table row of an earlier
/// instance whose `(pid, start_time)` still matches is asked to exit, forced
/// after the grace period, and its row cleared once the OS confirms the exit.
async fn reclaim_orphans(core: &CoreHandle) {
    let rows = match core.other_instance_processes().await {
        Ok(rows) => rows,
        Err(e) => {
            error!("cannot read the process table: {e}");
            return;
        }
    };
    for row in rows {
        let exited = loop {
            match uta_proc::reclaim(row.process, ORPHAN_GRACE).await {
                Ok(exited) => break exited,
                Err(e) => {
                    warn!(pid = row.process.pid(), "orphan not yet terminated: {e}");
                    tokio::time::sleep(ORPHAN_RETRY).await;
                }
            }
        };
        match core.clear_process(exited).await {
            Ok(()) => info!(pid = row.process.pid(), instance = %row.instance, "orphan reclaimed"),
            Err(e) => error!(pid = row.process.pid(), "clearing the process row failed: {e}"),
        }
    }
}

/// Resolves when the OS asks the core to stop (terminal interrupt or
/// service-manager termination).
async fn stop_requested() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            Err(e) => {
                warn!("cannot listen for SIGTERM ({e}); only SIGINT stops the core");
                wait_ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    wait_ctrl_c().await;
}

async fn wait_ctrl_c() {
    if let Err(e) = tokio::signal::ctrl_c().await {
        warn!("cannot listen for the interrupt signal ({e}); stopping now");
    }
}
