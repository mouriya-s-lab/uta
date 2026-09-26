//! Single-writer SQLite store of the UTA core (design §7.4).
//!
//! Ownership model:
//! - [`Store`] owns the only read-write `rusqlite::Connection`. It is `Send`
//!   but not `Sync`: the core thread creates it, uses it and drops it; nothing
//!   else ever holds a reference to the connection.
//! - Writes happen only inside [`Store::transact`]. The closure receives a
//!   [`Tx`] that borrows the connection for the closure's duration, so neither
//!   the transaction nor any table view can escape it.
//! - Table views ([`Instances`], [`Processes`]) expose exactly the operations
//!   the design allows on that table; there is no raw SQL entry point.
//! - A value leaves a write as [`Committed<T>`] only after `COMMIT` returned.
//!   In-memory state of the core is updated from `Committed` values, never
//!   from a closure that might still roll back.
//! - Durability: WAL with `synchronous=FULL`, so a returned commit has been
//!   synced to the WAL (design §7.4, §6.5 "durable append").

mod schema;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use uta_base::{IdError, IntegrationId, ProcessRole, ProgramId};
use uta_proc::{ExitConfirmed, OsProcessId};

pub use rusqlite::Error as SqliteError;
pub use schema::FORMAT_VERSION;

/// Why the store could not be opened. Each variant is a core-level refusal to
/// start (design §7.2 "失败分两级").
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    #[error("database file is format version {found}, newer than the supported {supported}")]
    FormatTooNew { found: u32, supported: u32 },
    #[error("migrating the database from format version {from} to {to} failed: {source}")]
    Migration {
        from: u32,
        to: u32,
        source: rusqlite::Error,
    },
    #[error("SQLite refused WAL journal mode (got {0:?})")]
    NotWal(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

/// A write that did not commit. Nothing it did is visible afterwards.
#[derive(Debug, thiserror::Error)]
pub enum TxError<E> {
    /// The closure returned an error; the transaction was rolled back.
    #[error("transaction aborted")]
    Aborted(E),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

/// A row read back from the store did not decode into its domain type.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error("process table row has unknown role kind {0:?}")]
    UnknownRoleKind(String),
    #[error("process table row has an invalid id: {0}")]
    InvalidId(#[from] IdError),
    #[error("process table row has a pid or start time out of range")]
    OutOfRange,
}

/// The result of a transaction that has committed.
///
/// Only [`Store::transact`] constructs it, after `COMMIT` returned.
#[derive(Debug)]
#[must_use = "committed values are what in-memory state may be updated from"]
pub struct Committed<T>(T);

impl<T> Committed<T> {
    pub fn into_inner(self) -> T {
        self.0
    }

    pub fn get(&self) -> &T {
        &self.0
    }
}

/// Milliseconds since the Unix epoch, read from the core's clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnixMillis(i64);

impl UnixMillis {
    pub fn now() -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        Self(millis)
    }

    pub fn get(self) -> i64 {
        self.0
    }
}

/// A core instance id (design §7.2 step 1): monotonically increasing, minted
/// only by [`Instances::begin`] inside the fence transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InstanceId(u64);

impl InstanceId {
    pub fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for InstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// The running instance's authority to write its own end anchor.
///
/// Minted once per process by the fence transaction and consumed by
/// [`Instances::end`], so an instance ends at most once and only itself.
#[derive(Debug)]
#[must_use = "the current instance must eventually be ended by a controlled stop"]
pub struct CurrentInstance {
    id: InstanceId,
}

impl CurrentInstance {
    pub fn id(&self) -> InstanceId {
        self.id
    }
}

/// How an earlier instance ended (design §7.5 实例表).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceEnd {
    /// A controlled stop wrote the end anchor.
    Stopped { at: UnixMillis },
    /// No end anchor: it crashed, and the successor's fence transaction is
    /// where it lost authority.
    NoEndAnchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceRecord {
    pub id: InstanceId,
    pub started_at: UnixMillis,
    pub end: InstanceEnd,
}

/// A process-table row: an index into an OS process (design §7.5 进程表).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRecord {
    pub instance: InstanceId,
    pub process: OsProcessId,
    pub role: ProcessRole,
}

/// The core's single SQLite file.
#[derive(Debug)]
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens (creating if absent) the database, enforces WAL + `synchronous=FULL`,
    /// and brings the format forward to [`FORMAT_VERSION`] in one transaction.
    pub fn open(path: &Path) -> Result<Self, OpenError> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(OpenError::NotWal(mode));
        }
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut store = Self { conn };
        schema::bring_forward(&mut store.conn)?;
        Ok(store)
    }

    /// Runs `f` in one `BEGIN IMMEDIATE` transaction. The result is returned as
    /// [`Committed`] only if `COMMIT` succeeded; if `f` fails, everything it did
    /// is rolled back.
    pub fn transact<T, E>(
        &mut self,
        f: impl FnOnce(&mut Tx<'_>) -> Result<T, E>,
    ) -> Result<Committed<T>, TxError<E>> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut tx = Tx { tx };
        let value = f(&mut tx).map_err(TxError::Aborted)?;
        tx.tx.commit()?;
        Ok(Committed(value))
    }

    /// The format version recorded in the file.
    pub fn format_version(&self) -> Result<u32, rusqlite::Error> {
        schema::recorded_version(&self.conn).map(|v| v.unwrap_or(0))
    }

    /// The most recent instance before `current`, if any.
    pub fn previous_instance(
        &self,
        current: &CurrentInstance,
    ) -> Result<Option<InstanceRecord>, ReadError> {
        self.conn
            .query_row(
                "SELECT instance_id, started_at_ms, ended_at_ms FROM instances \
                 WHERE instance_id < ?1 ORDER BY instance_id DESC LIMIT 1",
                params![to_sql_u64(current.id.0)?],
                instance_record,
            )
            .optional()?
            .transpose()
    }

    /// Process-table rows registered by instances other than `current`: the
    /// candidates for orphan reclamation (design §7.2 step 1).
    pub fn processes_of_other_instances(
        &self,
        current: &CurrentInstance,
    ) -> Result<Vec<ProcessRecord>, ReadError> {
        let mut stmt = self.conn.prepare(
            "SELECT instance_id, pid, start_time, role_kind, role_id FROM processes \
             WHERE instance_id <> ?1 ORDER BY instance_id, pid",
        )?;
        let rows = stmt.query_map(params![to_sql_u64(current.id.0)?], process_record)?;
        rows.map(|row| row?).collect()
    }

    /// Closes the connection on the calling (owning) thread.
    pub fn close(self) -> Result<(), rusqlite::Error> {
        self.conn.close().map_err(|(_, e)| e)
    }
}

/// An open write transaction; exists only inside [`Store::transact`].
#[derive(Debug)]
pub struct Tx<'c> {
    tx: rusqlite::Transaction<'c>,
}

impl Tx<'_> {
    pub fn instances(&mut self) -> Instances<'_> {
        Instances { tx: &self.tx }
    }

    pub fn processes(&mut self) -> Processes<'_> {
        Processes { tx: &self.tx }
    }
}

/// The instance table. Rows are inserted by the fence transaction and receive
/// at most one end anchor, written by the instance itself.
#[derive(Debug)]
pub struct Instances<'t> {
    tx: &'t rusqlite::Transaction<'t>,
}

impl Instances<'_> {
    /// Inserts the next instance row (design §7.2 step 1). Call only while the
    /// OS instance lock is held; the lock plus this row are the fence.
    pub fn begin(&mut self, started_at: UnixMillis) -> Result<CurrentInstance, rusqlite::Error> {
        let next: i64 = self.tx.query_row(
            "SELECT COALESCE(MAX(instance_id), 0) + 1 FROM instances",
            [],
            |r| r.get(0),
        )?;
        self.tx.execute(
            "INSERT INTO instances (instance_id, started_at_ms, ended_at_ms) VALUES (?1, ?2, NULL)",
            params![next, started_at.0],
        )?;
        let id =
            u64::try_from(next).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, next))?;
        Ok(CurrentInstance { id: InstanceId(id) })
    }

    /// Writes the end anchor of the current instance (controlled stop step 5).
    /// Consumes the token: an instance is ended at most once, and only by itself.
    pub fn end(&mut self, current: CurrentInstance, at: UnixMillis) -> Result<(), rusqlite::Error> {
        self.tx.execute(
            "UPDATE instances SET ended_at_ms = ?2 WHERE instance_id = ?1 AND ended_at_ms IS NULL",
            params![to_sql_u64(current.id.0)?, at.0],
        )?;
        Ok(())
    }
}

/// The process table: rows are written when a process is spawned and cleared
/// only with the OS's confirmation that the process has exited.
#[derive(Debug)]
pub struct Processes<'t> {
    tx: &'t rusqlite::Transaction<'t>,
}

impl Processes<'_> {
    pub fn register(
        &mut self,
        instance: &CurrentInstance,
        process: OsProcessId,
        role: &ProcessRole,
    ) -> Result<(), rusqlite::Error> {
        let (kind, id) = match role {
            ProcessRole::Integration(id) => (ROLE_INTEGRATION, id.as_str()),
            ProcessRole::ProgramHost(id) => (ROLE_PROGRAM_HOST, id.as_str()),
        };
        self.tx.execute(
            "INSERT INTO processes (pid, start_time, instance_id, role_kind, role_id) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                process.pid(),
                to_sql_u64(process.start_time())?,
                to_sql_u64(instance.id.0)?,
                kind,
                id
            ],
        )?;
        Ok(())
    }

    /// Clears the row of a process the OS confirmed as exited. Consumes the proof.
    pub fn clear(&mut self, exited: ExitConfirmed) -> Result<(), rusqlite::Error> {
        let process = exited.id();
        self.tx.execute(
            "DELETE FROM processes WHERE pid = ?1 AND start_time = ?2",
            params![process.pid(), to_sql_u64(process.start_time())?],
        )?;
        Ok(())
    }
}

const ROLE_INTEGRATION: &str = "integration";
const ROLE_PROGRAM_HOST: &str = "program_host";

fn to_sql_u64(v: u64) -> Result<i64, rusqlite::Error> {
    i64::try_from(v).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

fn from_sql_u64(v: i64) -> Result<u64, ReadError> {
    u64::try_from(v).map_err(|_| ReadError::OutOfRange)
}

fn instance_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<InstanceRecord, ReadError>> {
    let id: i64 = row.get(0)?;
    let started: i64 = row.get(1)?;
    let ended: Option<i64> = row.get(2)?;
    Ok(from_sql_u64(id).map(|id| InstanceRecord {
        id: InstanceId(id),
        started_at: UnixMillis(started),
        end: ended.map_or(InstanceEnd::NoEndAnchor, |at| InstanceEnd::Stopped {
            at: UnixMillis(at),
        }),
    }))
}

fn process_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<Result<ProcessRecord, ReadError>> {
    let instance: i64 = row.get(0)?;
    let pid: i64 = row.get(1)?;
    let start_time: i64 = row.get(2)?;
    let kind: String = row.get(3)?;
    let id: String = row.get(4)?;
    Ok((|| {
        let role = match kind.as_str() {
            ROLE_INTEGRATION => ProcessRole::Integration(IntegrationId::parse(&id)?),
            ROLE_PROGRAM_HOST => ProcessRole::ProgramHost(ProgramId::parse(&id)?),
            _ => return Err(ReadError::UnknownRoleKind(kind)),
        };
        let pid = u32::try_from(pid).map_err(|_| ReadError::OutOfRange)?;
        Ok(ProcessRecord {
            instance: InstanceId(from_sql_u64(instance)?),
            process: OsProcessId::from_row(pid, from_sql_u64(start_time)?),
            role,
        })
    })())
}
