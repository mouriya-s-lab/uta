//! The two journals' storage primitives (design core/core-process/design.md §3.1, §3.3; core/core-process/observation-journal.md §2.1; core/core-process/storage.md §2.2).
//!
//! - Observation streams are identified by `StreamId = (source, stream, epoch)`;
//!   the epoch and every `Seq` are allocated here, inside the transaction that
//!   uses them, so no other crate can fabricate a position.
//! - Execution-fact streams are append-only: [`Executions`] has `append` and
//!   nothing else. Observation streams may also be compacted ([`Observations`]
//!   exposes the deletion primitives; which records a stream may lose is the
//!   observation Journal element's rule, core/core-process/observation-journal.md §2.4).
//! - Record bodies are opaque bytes tagged with a [`RecordKind`]; the store
//!   never parses them (core/core-process/storage.md §2.2 "两侧共享存储原语，不共享抽象").

use rusqlite::{OptionalExtension, params};
use uta_base::{IntegrationId, ProgramId, Source, StreamName, WriteLaneKey};

use crate::ReadError;

/// A stream epoch, allocated by [`Observations::open_epoch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Epoch(u32);

impl Epoch {
    pub fn get(self) -> u32 {
        self.0
    }
}

/// A position within one stream; allocated by `append`, strictly increasing
/// per stream, never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seq(u64);

impl Seq {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// `StreamId = (source, stream, epoch)` of an observation stream (core/core-process/design.md §3.1).
/// Only the store constructs it: by opening an epoch, or by reading one back.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamId {
    source: Source,
    name: StreamName,
    epoch: Epoch,
}

impl StreamId {
    pub fn source(&self) -> &Source {
        &self.source
    }

    pub fn name(&self) -> &StreamName {
        &self.name
    }

    pub fn epoch(&self) -> Epoch {
        self.epoch
    }
}

/// `LogPosition = (StreamId, Seq)` of an observation record (core/core-process/design.md §3.1).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LogPosition {
    stream: StreamId,
    seq: Seq,
}

impl LogPosition {
    pub fn stream(&self) -> &StreamId {
        &self.stream
    }

    pub fn seq(&self) -> Seq {
        self.seq
    }
}

/// An execution-fact stream. These name streams the design defines; the
/// identity is not minted by anyone, only positions on it are.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExecStream {
    /// The one control stream per user state root (core/core-process/design.md §4.5 控制流).
    Control,
    /// An integration's declaration versions (core/core-process/integration-session.md §3.6).
    Declaration(IntegrationId),
    /// One write lane of an integration (core/core-process/lane.md §3.1).
    Lane {
        integration: IntegrationId,
        lane: WriteLaneKey,
    },
    /// A program's request stream (core/core-process/outbound-requests.md §3.4).
    Requests(ProgramId),
}

/// Position of an execution-fact record.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExecPosition {
    stream: ExecStream,
    seq: Seq,
}

impl ExecPosition {
    pub fn stream(&self) -> &ExecStream {
        &self.stream
    }

    pub fn seq(&self) -> Seq {
        self.seq
    }
}

/// The record type tag stored next to an opaque body. Element crates define
/// their kinds as constants; the store only stores and returns the tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecordKind(&'static str);

impl RecordKind {
    pub const fn new(tag: &'static str) -> Self {
        Self(tag)
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }
}

/// A committed record read back from a journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRecord {
    pub seq: Seq,
    pub kind: String,
    pub body: Vec<u8>,
}

/// Observation journal view of an open transaction.
#[derive(Debug)]
pub struct Observations<'t> {
    pub(crate) tx: &'t rusqlite::Transaction<'t>,
}

impl Observations<'_> {
    /// Opens the next epoch of `(source, name)` and returns its identity. The
    /// caller appends the epoch's first record (its `Gap{origin: Source}`) in
    /// the same transaction.
    pub fn open_epoch(
        &mut self,
        source: &Source,
        name: &StreamName,
    ) -> Result<StreamId, rusqlite::Error> {
        let (kind, id) = source_columns(source);
        let next: i64 = self.tx.query_row(
            "SELECT COALESCE(MAX(epoch), 0) + 1 FROM obs_streams \
             WHERE source_kind = ?1 AND source_id = ?2 AND name = ?3",
            params![kind, id, name.as_str()],
            |r| r.get(0),
        )?;
        let epoch = Epoch(to_u32(next)?);
        self.tx.execute(
            "INSERT INTO obs_streams (source_kind, source_id, name, epoch, next_seq) \
             VALUES (?1, ?2, ?3, ?4, 1)",
            params![kind, id, name.as_str(), next],
        )?;
        Ok(StreamId {
            source: source.clone(),
            name: name.clone(),
            epoch,
        })
    }

    /// The latest epoch of `(source, name)`, if one was opened.
    pub fn current_epoch(
        &self,
        source: &Source,
        name: &StreamName,
    ) -> Result<Option<StreamId>, rusqlite::Error> {
        let (kind, id) = source_columns(source);
        let epoch: Option<i64> = self.tx.query_row(
            "SELECT MAX(epoch) FROM obs_streams \
             WHERE source_kind = ?1 AND source_id = ?2 AND name = ?3",
            params![kind, id, name.as_str()],
            |r| r.get(0),
        )?;
        epoch
            .map(|e| {
                Ok(StreamId {
                    source: source.clone(),
                    name: name.clone(),
                    epoch: Epoch(to_u32(e)?),
                })
            })
            .transpose()
    }

    /// Appends one record and allocates its `Seq`.
    pub fn append(
        &mut self,
        stream: &StreamId,
        kind: RecordKind,
        body: &[u8],
    ) -> Result<LogPosition, rusqlite::Error> {
        let key = obs_stream_key(self.tx, stream)?;
        let seq = take_seq(self.tx, Side::Observation, key)?;
        self.tx.execute(
            "INSERT INTO obs_records (stream_key, seq, kind, body) VALUES (?1, ?2, ?3, ?4)",
            params![key, to_i64(seq.0)?, kind.0, body],
        )?;
        Ok(LogPosition {
            stream: stream.clone(),
            seq,
        })
    }

    /// Compaction primitive: deletes every record of `stream` below `boundary`.
    pub fn delete_below(
        &mut self,
        stream: &StreamId,
        boundary: Seq,
    ) -> Result<usize, rusqlite::Error> {
        let key = obs_stream_key(self.tx, stream)?;
        self.tx.execute(
            "DELETE FROM obs_records WHERE stream_key = ?1 AND seq < ?2",
            params![key, to_i64(boundary.0)?],
        )
    }

    /// Compaction primitive: deletes one record (a superseded one, core/core-process/observation-journal.md §2.4).
    /// Returns whether a record was deleted.
    pub fn delete(&mut self, position: &LogPosition) -> Result<bool, rusqlite::Error> {
        let key = obs_stream_key(self.tx, &position.stream)?;
        let n = self.tx.execute(
            "DELETE FROM obs_records WHERE stream_key = ?1 AND seq = ?2",
            params![key, to_i64(position.seq.0)?],
        )?;
        Ok(n == 1)
    }
}

/// Execution-fact journal view of an open transaction: append only.
#[derive(Debug)]
pub struct Executions<'t> {
    pub(crate) tx: &'t rusqlite::Transaction<'t>,
}

impl Executions<'_> {
    /// Appends one record and allocates its `Seq`. The stream is created on
    /// its first record.
    pub fn append(
        &mut self,
        stream: &ExecStream,
        kind: RecordKind,
        body: &[u8],
    ) -> Result<ExecPosition, rusqlite::Error> {
        let (skind, owner, lane) = exec_columns(stream);
        self.tx.execute(
            "INSERT INTO exec_streams (kind, owner_id, lane, next_seq) VALUES (?1, ?2, ?3, 1) \
             ON CONFLICT (kind, owner_id, lane) DO NOTHING",
            params![skind, owner, lane],
        )?;
        let key: i64 = self.tx.query_row(
            "SELECT stream_key FROM exec_streams WHERE kind = ?1 AND owner_id = ?2 AND lane = ?3",
            params![skind, owner, lane],
            |r| r.get(0),
        )?;
        let seq = take_seq(self.tx, Side::Execution, key)?;
        self.tx.execute(
            "INSERT INTO exec_records (stream_key, seq, kind, body) VALUES (?1, ?2, ?3, ?4)",
            params![key, to_i64(seq.0)?, kind.0, body],
        )?;
        Ok(ExecPosition {
            stream: stream.clone(),
            seq,
        })
    }
}

pub(crate) fn read_observations(
    conn: &rusqlite::Connection,
    stream: &StreamId,
    after: Option<Seq>,
    limit: usize,
) -> Result<Vec<StoredRecord>, ReadError> {
    let Some(key) = obs_stream_key_opt(conn, stream)? else {
        return Ok(Vec::new());
    };
    read_records(conn, Side::Observation, key, after, limit)
}

pub(crate) fn read_executions(
    conn: &rusqlite::Connection,
    stream: &ExecStream,
    after: Option<Seq>,
    limit: usize,
) -> Result<Vec<StoredRecord>, ReadError> {
    let (skind, owner, lane) = exec_columns(stream);
    let key: Option<i64> = conn
        .query_row(
            "SELECT stream_key FROM exec_streams WHERE kind = ?1 AND owner_id = ?2 AND lane = ?3",
            params![skind, owner, lane],
            |r| r.get(0),
        )
        .optional()?;
    match key {
        Some(key) => read_records(conn, Side::Execution, key, after, limit),
        None => Ok(Vec::new()),
    }
}

/// Which journal a record table belongs to; selects fixed SQL text, so no
/// statement is assembled at runtime.
#[derive(Debug, Clone, Copy)]
enum Side {
    Observation,
    Execution,
}

impl Side {
    fn read_sql(self) -> &'static str {
        match self {
            Side::Observation => {
                "SELECT seq, kind, body FROM obs_records \
                 WHERE stream_key = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3"
            }
            Side::Execution => {
                "SELECT seq, kind, body FROM exec_records \
                 WHERE stream_key = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3"
            }
        }
    }

    /// Takes the counter's current value and advances it, unless it would
    /// leave the INTEGER range (SQLite would silently turn it into a REAL).
    fn take_seq_sql(self) -> &'static str {
        match self {
            Side::Observation => {
                "UPDATE obs_streams SET next_seq = next_seq + 1 \
                 WHERE stream_key = ?1 AND next_seq < 9223372036854775807 RETURNING next_seq - 1"
            }
            Side::Execution => {
                "UPDATE exec_streams SET next_seq = next_seq + 1 \
                 WHERE stream_key = ?1 AND next_seq < 9223372036854775807 RETURNING next_seq - 1"
            }
        }
    }
}

fn read_records(
    conn: &rusqlite::Connection,
    side: Side,
    key: i64,
    after: Option<Seq>,
    limit: usize,
) -> Result<Vec<StoredRecord>, ReadError> {
    let after = to_i64(after.map_or(0, |s| s.0))?;
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let mut stmt = conn.prepare_cached(side.read_sql())?;
    let rows = stmt.query_map(params![key, after, limit], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Vec<u8>>(2)?,
        ))
    })?;
    rows.map(|row| {
        let (seq, kind, body) = row?;
        Ok(StoredRecord {
            seq: Seq(u64::try_from(seq).map_err(|_| ReadError::OutOfRange)?),
            kind,
            body,
        })
    })
    .collect()
}

/// Allocates the stream's next `Seq` (per-stream counter, so deleting records
/// never lets a sequence number be reused). The stream row must exist; when
/// its counter is exhausted nothing is updated and allocation fails.
fn take_seq(tx: &rusqlite::Transaction<'_>, side: Side, key: i64) -> Result<Seq, rusqlite::Error> {
    let seq: Option<i64> = tx
        .prepare_cached(side.take_seq_sql())?
        .query_row(params![key], |r| r.get(0))
        .optional()?;
    let seq = seq.ok_or(rusqlite::Error::IntegralValueOutOfRange(0, i64::MAX))?;
    Ok(Seq(u64::try_from(seq).map_err(|_| {
        rusqlite::Error::IntegralValueOutOfRange(0, seq)
    })?))
}

fn obs_stream_key(
    tx: &rusqlite::Transaction<'_>,
    stream: &StreamId,
) -> Result<i64, rusqlite::Error> {
    obs_stream_key_opt(tx, stream)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

fn obs_stream_key_opt(
    conn: &rusqlite::Connection,
    stream: &StreamId,
) -> Result<Option<i64>, rusqlite::Error> {
    let (kind, id) = source_columns(&stream.source);
    conn.query_row(
        "SELECT stream_key FROM obs_streams \
         WHERE source_kind = ?1 AND source_id = ?2 AND name = ?3 AND epoch = ?4",
        params![kind, id, stream.name.as_str(), stream.epoch.0],
        |r| r.get(0),
    )
    .optional()
}

fn source_columns(source: &Source) -> (&'static str, &str) {
    match source {
        Source::Integration(id) => ("integration", id.as_str()),
        Source::Program(id) => ("program", id.as_str()),
    }
}

fn exec_columns(stream: &ExecStream) -> (&'static str, &str, &str) {
    match stream {
        ExecStream::Control => ("control", "", ""),
        ExecStream::Declaration(id) => ("declaration", id.as_str(), ""),
        ExecStream::Lane { integration, lane } => ("lane", integration.as_str(), lane.as_str()),
        ExecStream::Requests(id) => ("requests", id.as_str(), ""),
    }
}

fn to_i64(v: u64) -> Result<i64, rusqlite::Error> {
    i64::try_from(v).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

fn to_u32(v: i64) -> Result<u32, rusqlite::Error> {
    u32::try_from(v).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(0, v))
}
