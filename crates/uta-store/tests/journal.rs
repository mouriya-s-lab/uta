use uta_base::{IntegrationId, Source, StreamName, WriteLaneKey};
use uta_store::{ExecStream, RecordKind, Store, TxError};

const GAP: RecordKind = RecordKind::new("gap_source");
const QUOTE: RecordKind = RecordKind::new("quote");
const FACT: RecordKind = RecordKind::new("fact");

fn source() -> Source {
    Source::Integration(IntegrationId::parse("okx").unwrap())
}

fn name() -> StreamName {
    StreamName::parse("quotes").unwrap()
}

#[test]
fn observation_positions_are_allocated_in_the_transaction_and_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("uta.db");
    let mut store = Store::open(&db).unwrap();

    let (first_epoch, positions) = store
        .transact(|tx| {
            let mut obs = tx.observations();
            let stream = obs.open_epoch(&source(), &name())?;
            let gap = obs.append(&stream, GAP, b"start")?;
            let quote = obs.append(&stream, QUOTE, b"q1")?;
            Ok::<_, uta_store::SqliteError>((stream, vec![gap, quote]))
        })
        .unwrap()
        .into_inner();
    assert_eq!(first_epoch.epoch().get(), 1);
    assert_eq!(
        positions.iter().map(|p| p.seq().get()).collect::<Vec<_>>(),
        [1, 2]
    );

    // A rolled-back append leaves nothing behind.
    let aborted = store.transact(|tx| {
        tx.observations().append(&first_epoch, QUOTE, b"lost")?;
        Err::<(), _>(uta_store::SqliteError::InvalidQuery)
    });
    assert!(matches!(aborted, Err(TxError::Aborted(_))));

    // Compaction below the boundary never lets a sequence number be reused.
    let after_compaction = store
        .transact(|tx| {
            let mut obs = tx.observations();
            let deleted = obs.delete_below(&first_epoch, positions[1].seq())?;
            let next = obs.append(&first_epoch, QUOTE, b"q2")?;
            let second_epoch = obs.open_epoch(&source(), &name())?;
            Ok::<_, uta_store::SqliteError>((deleted, next.seq().get(), second_epoch))
        })
        .unwrap()
        .into_inner();
    assert_eq!(
        after_compaction.0, 1,
        "only the record below the boundary is deleted"
    );
    assert_eq!(after_compaction.1, 3);
    assert_eq!(after_compaction.2.epoch().get(), 2);
    store.close().unwrap();

    let store = Store::open(&db).unwrap();
    let records = store.read_observations(&first_epoch, None, 100).unwrap();
    let seen: Vec<(u64, &str, &[u8])> = records
        .iter()
        .map(|r| (r.seq.get(), r.kind.as_str(), r.body.as_slice()))
        .collect();
    assert_eq!(seen, [(2, "quote", &b"q1"[..]), (3, "quote", &b"q2"[..])]);
    let tail = store
        .read_observations(&first_epoch, Some(positions[1].seq()), 100)
        .unwrap();
    assert_eq!(
        tail.len(),
        1,
        "reading after a position returns only later records"
    );
}

#[test]
fn execution_streams_number_independently_and_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("uta.db");
    let lane = ExecStream::Lane {
        integration: IntegrationId::parse("okx").unwrap(),
        lane: WriteLaneKey::parse("acct-1").unwrap(),
    };
    let mut store = Store::open(&db).unwrap();
    let seqs = store
        .transact(|tx| {
            let mut exec = tx.executions();
            let a = exec.append(&ExecStream::Control, FACT, b"c1")?;
            let b = exec.append(&lane, FACT, b"l1")?;
            let c = exec.append(&ExecStream::Control, FACT, b"c2")?;
            Ok::<_, uta_store::SqliteError>([a.seq().get(), b.seq().get(), c.seq().get()])
        })
        .unwrap()
        .into_inner();
    assert_eq!(seqs, [1, 1, 2]);
    store.close().unwrap();

    let store = Store::open(&db).unwrap();
    let control: Vec<Vec<u8>> = store
        .read_executions(&ExecStream::Control, None, 10)
        .unwrap()
        .into_iter()
        .map(|r| r.body)
        .collect();
    assert_eq!(control, [b"c1".to_vec(), b"c2".to_vec()]);
    assert_eq!(store.read_executions(&lane, None, 10).unwrap().len(), 1);
}
