//! Format versions and forward-only migrations (design core/core-process/storage.md §2.5, C14).
//!
//! `MIGRATIONS[n]` brings a file from format version `n` to `n + 1`. All pending
//! migrations run in one transaction, so after a crash at any point the file is
//! either at its old version or at the new one, never in between.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::OpenError;

/// Format version this build writes and reads.
pub const FORMAT_VERSION: u32 = 1;

const MIGRATIONS: [&str; FORMAT_VERSION as usize] = [
    // 0 -> 1
    "CREATE TABLE schema_meta (
         singleton      INTEGER PRIMARY KEY CHECK (singleton = 1),
         format_version INTEGER NOT NULL
     );
     CREATE TABLE instances (
         instance_id   INTEGER PRIMARY KEY CHECK (instance_id > 0),
         started_at_ms INTEGER NOT NULL,
         ended_at_ms   INTEGER
     );
     CREATE TABLE processes (
         pid         INTEGER NOT NULL,
         start_time  INTEGER NOT NULL,
         instance_id INTEGER NOT NULL REFERENCES instances (instance_id),
         role_kind   TEXT NOT NULL CHECK (role_kind IN ('integration', 'program_host')),
         role_id     TEXT NOT NULL,
         PRIMARY KEY (pid, start_time)
     );
     CREATE TABLE obs_streams (
         stream_key  INTEGER PRIMARY KEY,
         source_kind TEXT NOT NULL CHECK (source_kind IN ('integration', 'program')),
         source_id   TEXT NOT NULL,
         name        TEXT NOT NULL,
         epoch       INTEGER NOT NULL CHECK (epoch BETWEEN 1 AND 4294967295),
         next_seq    INTEGER NOT NULL CHECK (next_seq > 0),
         UNIQUE (source_kind, source_id, name, epoch)
     );
     CREATE TABLE obs_records (
         stream_key INTEGER NOT NULL REFERENCES obs_streams (stream_key),
         seq        INTEGER NOT NULL CHECK (seq > 0),
         kind       TEXT NOT NULL,
         body       BLOB NOT NULL,
         PRIMARY KEY (stream_key, seq)
     ) WITHOUT ROWID;
     CREATE TABLE exec_streams (
         stream_key INTEGER PRIMARY KEY,
         kind       TEXT NOT NULL CHECK (kind IN ('control', 'declaration', 'lane', 'requests')),
         owner_id   TEXT NOT NULL,
         lane       TEXT NOT NULL,
         next_seq   INTEGER NOT NULL CHECK (next_seq > 0),
         UNIQUE (kind, owner_id, lane)
     );
     CREATE TABLE exec_records (
         stream_key INTEGER NOT NULL REFERENCES exec_streams (stream_key),
         seq        INTEGER NOT NULL CHECK (seq > 0),
         kind       TEXT NOT NULL,
         body       BLOB NOT NULL,
         PRIMARY KEY (stream_key, seq)
     ) WITHOUT ROWID;",
];

/// What the file says about its format.
#[derive(Debug)]
pub(crate) enum Recorded {
    /// No meta table: a new (or foreign, empty) file.
    Fresh,
    Version(u32),
    /// The meta table exists but does not hold a valid version.
    Unreadable(String),
}

pub(crate) fn recorded_version(conn: &Connection) -> rusqlite::Result<Recorded> {
    let has_meta: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_meta')",
        [],
        |r| r.get(0),
    )?;
    if !has_meta {
        return Ok(Recorded::Fresh);
    }
    let value = conn
        .query_row(
            "SELECT format_version FROM schema_meta WHERE singleton = 1",
            [],
            |r| r.get::<_, rusqlite::types::Value>(0),
        )
        .optional()?;
    Ok(match value {
        None => Recorded::Unreadable("schema_meta has no row".to_owned()),
        Some(rusqlite::types::Value::Integer(v)) => match u32::try_from(v) {
            Ok(v) => Recorded::Version(v),
            Err(_) => Recorded::Unreadable(format!("format version {v} is out of range")),
        },
        Some(other) => Recorded::Unreadable(format!("format version is not an integer: {other:?}")),
    })
}

pub(crate) fn bring_forward(conn: &mut Connection) -> Result<(), OpenError> {
    let found = match recorded_version(conn)? {
        Recorded::Fresh => 0,
        Recorded::Version(v) => v,
        Recorded::Unreadable(reason) => return Err(OpenError::FormatUnreadable(reason)),
    };
    if found > FORMAT_VERSION {
        return Err(OpenError::FormatTooNew {
            found,
            supported: FORMAT_VERSION,
        });
    }
    if found == FORMAT_VERSION {
        return Ok(());
    }
    let migrate = |conn: &mut Connection| -> rusqlite::Result<()> {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for step in &MIGRATIONS[found as usize..] {
            tx.execute_batch(step)?;
        }
        tx.execute(
            "INSERT INTO schema_meta (singleton, format_version) VALUES (1, ?1)
             ON CONFLICT (singleton) DO UPDATE SET format_version = excluded.format_version",
            params![FORMAT_VERSION],
        )?;
        tx.commit()
    };
    migrate(conn).map_err(|source| OpenError::Migration {
        from: found,
        to: FORMAT_VERSION,
        source,
    })
}
