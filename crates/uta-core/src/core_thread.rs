//! The core thread: the only owner of the SQLite connection and of the
//! instance's durable state.
//!
//! Other parts of the process reach it through [`CoreHandle`] by sending
//! closed, typed [`Command`]s over a bounded channel. Each command runs to
//! completion on the core thread (a write command is one transaction; a read
//! command is one statement) and replies over a oneshot. No closure,
//! connection or transaction crosses the thread boundary.
//!
//! Lifecycle, as handshakes:
//! - start: the thread opens the store and commits the fence transaction
//!   (instance row) before [`start`] returns a handle; on failure no handle
//!   exists and the thread has ended.
//! - stop: [`CoreHandle::stop`] consumes the handle; the thread writes the
//!   instance end anchor, closes the connection on itself, replies, and is
//!   joined before `stop` returns. It blocks and is called outside the async
//!   runtime, so it cannot be cancelled halfway.
//! - drop without stop: the handle closes the command channel and joins the
//!   thread, which drops the store on itself without an end anchor (the crash
//!   path of design core/core-process/design.md §4.7.4). The thread therefore never outlives its handle, and
//!   the OS lock (declared before the handle) is released only after it.

use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::thread::{self, JoinHandle};

use tokio::sync::{mpsc, oneshot};
use uta_proc::ExitConfirmed;
use uta_store::{
    ClockError, Committed, CurrentInstance, EndError, InstanceId, InstanceRecord, OpenError,
    ProcessRecord, ReadError, SqliteError, Store, TxError, UnixMillis,
};

/// Bound on queued commands; senders wait when the core thread is behind.
const COMMAND_QUEUE: usize = 64;

/// What the core knows once the fence is taken.
#[derive(Debug)]
pub struct Ready {
    pub instance: InstanceId,
    pub previous: Option<InstanceRecord>,
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("cannot open the store: {0}")]
    Open(#[from] OpenError),
    #[error("cannot read the core clock: {0}")]
    Clock(ClockError),
    #[error("the fence transaction did not commit: {0}")]
    Fence(SqliteError),
    #[error("cannot read the previous instance: {0}")]
    Read(#[from] ReadError),
    #[error("cannot start the core thread: {0}")]
    Spawn(std::io::Error),
    #[error("the core thread ended before reporting readiness")]
    Vanished,
}

/// Why a controlled stop did not complete. Whether the end anchor exists is
/// part of the answer, because the exit code reports it.
#[derive(Debug, thiserror::Error)]
pub enum StopError {
    #[error("cannot read the core clock; no end anchor written: {0}")]
    Clock(ClockError),
    #[error("the end anchor did not commit: {0}")]
    EndAnchor(EndError),
    #[error("the core thread ended without completing the stop; no end anchor confirmed")]
    CoreGone,
    #[error("the end anchor was written, but closing the store failed: {0}")]
    CloseAfterAnchor(SqliteError),
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error(transparent)]
    Write(#[from] TxError<SqliteError>),
    /// The core thread is gone; the command did not run.
    #[error("the core thread is not running")]
    CoreGone,
}

/// Commands the core thread executes. Each variant carries owned values only.
#[derive(Debug)]
enum Command {
    OtherInstanceProcesses {
        reply: oneshot::Sender<Result<Vec<ProcessRecord>, ReadError>>,
    },
    ClearProcess {
        exited: ExitConfirmed,
        reply: oneshot::Sender<Result<(), TxError<SqliteError>>>,
    },
    Stop {
        reply: oneshot::Sender<Result<(), StopError>>,
    },
}

/// Durable state owned by the core thread for the instance's lifetime.
#[derive(Debug)]
struct CoreState {
    store: Store,
    current: CurrentInstance,
}

/// Sends commands to the core thread and owns the thread's lifetime.
#[derive(Debug)]
pub struct CoreHandle {
    commands: Option<mpsc::Sender<Command>>,
    thread: Option<JoinHandle<()>>,
}

/// Opens the store on a new core thread and commits the fence transaction.
/// Call only while holding the [`crate::fence::InstanceLock`].
pub fn start(db: PathBuf) -> Result<(CoreHandle, Ready), StartError> {
    let (ready_tx, ready_rx) = std_mpsc::sync_channel(1);
    let (commands, inbox) = mpsc::channel(COMMAND_QUEUE);
    let thread = thread::Builder::new()
        .name("uta-core".into())
        .spawn(move || run(db, ready_tx, inbox))
        .map_err(StartError::Spawn)?;
    match ready_rx.recv() {
        Ok(Ok(ready)) => Ok((
            CoreHandle {
                commands: Some(commands),
                thread: Some(thread),
            },
            ready,
        )),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => {
            let _ = thread.join();
            Err(StartError::Vanished)
        }
    }
}

impl CoreHandle {
    /// Process-table rows registered by earlier instances (orphan candidates).
    pub async fn other_instance_processes(&self) -> Result<Vec<ProcessRecord>, CommandError> {
        let (reply, answer) = oneshot::channel();
        self.send(Command::OtherInstanceProcesses { reply }).await?;
        Ok(answer.await.map_err(|_| CommandError::CoreGone)??)
    }

    /// Clears a process-table row; the OS's exit confirmation is consumed.
    pub async fn clear_process(&self, exited: ExitConfirmed) -> Result<(), CommandError> {
        let (reply, answer) = oneshot::channel();
        self.send(Command::ClearProcess { exited, reply }).await?;
        Ok(answer.await.map_err(|_| CommandError::CoreGone)??)
    }

    /// Controlled stop, instance part (design core/core-process/design.md §4.7.4 受控停止 step 5 and closing
    /// SQLite): end anchor, close, join the thread. Blocking; call it outside
    /// the async runtime.
    pub fn stop(mut self) -> Result<(), StopError> {
        let result = match self.commands.take() {
            Some(commands) => {
                let (reply, answer) = oneshot::channel();
                match commands.blocking_send(Command::Stop { reply }) {
                    Ok(()) => answer.blocking_recv().unwrap_or(Err(StopError::CoreGone)),
                    Err(_) => Err(StopError::CoreGone),
                }
            }
            None => Err(StopError::CoreGone),
        };
        match (result, self.join()) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(e), _) => Err(e),
            (Ok(()), Err(())) => Err(StopError::CoreGone),
        }
    }

    async fn send(&self, command: Command) -> Result<(), CommandError> {
        match &self.commands {
            Some(commands) => commands
                .send(command)
                .await
                .map_err(|_| CommandError::CoreGone),
            None => Err(CommandError::CoreGone),
        }
    }

    /// Closes the command channel and waits for the thread to end.
    fn join(&mut self) -> Result<(), ()> {
        self.commands = None;
        match self.thread.take() {
            Some(thread) => thread.join().map_err(|_| ()),
            None => Ok(()),
        }
    }
}

impl Drop for CoreHandle {
    fn drop(&mut self) {
        let _ = self.join();
    }
}

fn run(
    db: PathBuf,
    ready: std_mpsc::SyncSender<Result<Ready, StartError>>,
    mut inbox: mpsc::Receiver<Command>,
) {
    let state = match open(db) {
        Ok((state, report)) => {
            if ready.send(Ok(report)).is_err() {
                return;
            }
            state
        }
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    serve(state, &mut inbox);
}

fn open(db: PathBuf) -> Result<(CoreState, Ready), StartError> {
    let mut store = Store::open(&db)?;
    let now = UnixMillis::now().map_err(StartError::Clock)?;
    let current = store.begin_instance(now).map_err(StartError::Fence)?;
    let previous = store.previous_instance(&current)?;
    let ready = Ready {
        instance: current.id(),
        previous,
    };
    Ok((CoreState { store, current }, ready))
}

fn serve(mut state: CoreState, inbox: &mut mpsc::Receiver<Command>) {
    while let Some(command) = inbox.blocking_recv() {
        match command {
            Command::OtherInstanceProcesses { reply } => {
                let _ = reply.send(state.store.processes_of_other_instances(&state.current));
            }
            Command::ClearProcess { exited, reply } => {
                let result = state
                    .store
                    .transact(|tx| tx.processes().clear(exited))
                    .map(Committed::into_inner);
                let _ = reply.send(result);
            }
            Command::Stop { reply } => {
                let _ = reply.send(state.stop());
                return;
            }
        }
    }
    // Every handle is gone without a stop: the instance ends like a crash,
    // without an end anchor (design core/core-process/design.md §4.7.4 "崩溃路径不变"); the store is dropped
    // (and its connection closed) here, on the owning thread.
}

impl CoreState {
    fn stop(self) -> Result<(), StopError> {
        let CoreState { mut store, current } = self;
        let now = UnixMillis::now().map_err(StopError::Clock)?;
        store
            .end_instance(current, now)
            .map_err(StopError::EndAnchor)?;
        store.close().map_err(StopError::CloseAfterAnchor)
    }
}
