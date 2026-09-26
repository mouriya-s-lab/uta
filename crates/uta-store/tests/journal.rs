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

    let (first_epoch, first_positions) = store
        .transact(|tx| {
            let mut obs = tx.observations();
            let stream = obs.open_epoch(&source(), &name())?;
            let gap = obs.append(&stream, GAP, b"start")?;
            let quote = obs.append(&stream, QUOTE, b"q1")?;
            Ok::<_, uta_store::SqliteError>((stream, [gap, quote]))
        })
        .unwrap()
        .into_inner();
    assert_eq!(first_epoch.epoch().get(), 1);
    assert_eq!(
        first_positions
            .iter()
            .map(|position| position.seq().get())
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(first_positions[0].stream(), &first_epoch);
    assert_eq!(first_positions[1].stream(), &first_epoch);

    // A rolled-back append leaves nothing behind.
    let aborted = store.transact(|tx| {
        tx.observations().append(&first_epoch, QUOTE, b"lost")?;
        Err::<(), _>(uta_store::SqliteError::InvalidQuery)
    });
    assert!(matches!(aborted, Err(TxError::Aborted(_))));

    // A new epoch starts its own sequence at one and gets its gap in this
    // transaction. Deleting seq 2 from the first epoch must not delete seq 2
    // from the second epoch, and the next append must not reuse seq 2.
    let (
        second_epoch,
        second_gap,
        second_quote,
        latest_epoch,
        deleted,
        deleted_again,
        after_delete,
    ) = store
        .transact(|tx| {
            let mut obs = tx.observations();
            let second_epoch = obs.open_epoch(&source(), &name())?;
            let second_gap = obs.append(&second_epoch, GAP, b"second-start")?;
            let second_quote = obs.append(&second_epoch, QUOTE, b"second-quote")?;
            let latest_epoch = obs.current_epoch(&source(), &name())?;
            let deleted = obs.delete(&first_positions[1])?;
            let deleted_again = obs.delete(&first_positions[1])?;
            let after_delete = obs.append(&first_epoch, QUOTE, b"q2")?;
            Ok::<_, uta_store::SqliteError>((
                second_epoch,
                second_gap,
                second_quote,
                latest_epoch,
                deleted,
                deleted_again,
                after_delete,
            ))
        })
        .unwrap()
        .into_inner();

    assert_eq!(second_epoch.epoch().get(), 2);
    assert_eq!(latest_epoch.as_ref(), Some(&second_epoch));
    assert_eq!(second_gap.seq().get(), 1);
    assert_eq!(second_quote.seq().get(), 2);
    assert_eq!(second_gap.stream(), &second_epoch);
    assert_eq!(second_quote.stream(), &second_epoch);
    assert!(deleted, "the target record is removed");
    assert!(
        !deleted_again,
        "deleting the same position again is a no-op"
    );
    assert_eq!(after_delete.seq().get(), 3, "deleted seqs are never reused");
    assert_eq!(after_delete.stream(), &first_epoch);

    let tail = store
        .read_observations(&first_epoch, Some(first_positions[0].seq()), 100)
        .unwrap();
    assert_eq!(
        tail.iter()
            .map(|record| (
                record.seq.get(),
                record.kind.as_str(),
                record.body.as_slice()
            ))
            .collect::<Vec<_>>(),
        [(3, "quote", &b"q2"[..])]
    );
    store.close().unwrap();

    let mut store = Store::open(&db).unwrap();
    let resumed_positions = store
        .transact(|tx| {
            let mut obs = tx.observations();
            Ok::<_, uta_store::SqliteError>([
                obs.append(&first_epoch, QUOTE, b"q3")?,
                obs.append(&second_epoch, QUOTE, b"second-q2")?,
            ])
        })
        .unwrap()
        .into_inner();
    assert_eq!(
        resumed_positions
            .iter()
            .map(|position| position.seq().get())
            .collect::<Vec<_>>(),
        [4, 3]
    );
    assert_eq!(resumed_positions[0].stream(), &first_epoch);
    assert_eq!(resumed_positions[1].stream(), &second_epoch);

    let first_records = store.read_observations(&first_epoch, None, 100).unwrap();
    assert_eq!(
        first_records
            .iter()
            .map(|record| (
                record.seq.get(),
                record.kind.as_str(),
                record.body.as_slice()
            ))
            .collect::<Vec<_>>(),
        [
            (1, "gap_source", &b"start"[..]),
            (3, "quote", &b"q2"[..]),
            (4, "quote", &b"q3"[..]),
        ]
    );
    let second_records = store.read_observations(&second_epoch, None, 100).unwrap();
    assert_eq!(
        second_records
            .iter()
            .map(|record| (
                record.seq.get(),
                record.kind.as_str(),
                record.body.as_slice()
            ))
            .collect::<Vec<_>>(),
        [
            (1, "gap_source", &b"second-start"[..]),
            (2, "quote", &b"second-quote"[..]),
            (3, "quote", &b"second-q2"[..]),
        ]
    );
}

#[test]
fn execution_streams_number_independently_and_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("uta.db");
    let control = ExecStream::Control;
    let lane = ExecStream::Lane {
        integration: IntegrationId::parse("okx").unwrap(),
        lane: WriteLaneKey::parse("acct-1").unwrap(),
    };
    let mut store = Store::open(&db).unwrap();
    let positions = store
        .transact(|tx| {
            let mut exec = tx.executions();
            Ok::<_, uta_store::SqliteError>([
                exec.append(&control, FACT, b"c1")?,
                exec.append(&lane, FACT, b"l1")?,
                exec.append(&control, FACT, b"c2")?,
            ])
        })
        .unwrap()
        .into_inner();
    assert_eq!(
        positions
            .iter()
            .map(|position| position.seq().get())
            .collect::<Vec<_>>(),
        [1, 1, 2]
    );
    assert_eq!(positions[0].stream(), &control);
    assert_eq!(positions[1].stream(), &lane);
    assert_eq!(positions[2].stream(), &control);
    store.close().unwrap();

    let mut store = Store::open(&db).unwrap();
    let resumed_positions = store
        .transact(|tx| {
            let mut exec = tx.executions();
            Ok::<_, uta_store::SqliteError>([
                exec.append(&control, FACT, b"c3")?,
                exec.append(&lane, FACT, b"l2")?,
            ])
        })
        .unwrap()
        .into_inner();
    assert_eq!(
        resumed_positions
            .iter()
            .map(|position| position.seq().get())
            .collect::<Vec<_>>(),
        [3, 2]
    );
    assert_eq!(resumed_positions[0].stream(), &control);
    assert_eq!(resumed_positions[1].stream(), &lane);

    let control_records = store.read_executions(&control, None, 10).unwrap();
    assert_eq!(
        control_records
            .iter()
            .map(|record| (
                record.seq.get(),
                record.kind.as_str(),
                record.body.as_slice()
            ))
            .collect::<Vec<_>>(),
        [
            (1, "fact", &b"c1"[..]),
            (2, "fact", &b"c2"[..]),
            (3, "fact", &b"c3"[..]),
        ]
    );
    let lane_records = store.read_executions(&lane, None, 10).unwrap();
    assert_eq!(
        lane_records
            .iter()
            .map(|record| (
                record.seq.get(),
                record.kind.as_str(),
                record.body.as_slice()
            ))
            .collect::<Vec<_>>(),
        [(1, "fact", &b"l1"[..]), (2, "fact", &b"l2"[..])]
    );

    let unknown_lane = ExecStream::Lane {
        integration: IntegrationId::parse("okx").unwrap(),
        lane: WriteLaneKey::parse("acct-2").unwrap(),
    };
    assert!(
        store
            .read_executions(&unknown_lane, None, 10)
            .unwrap()
            .is_empty(),
        "reading an unknown stream returns no records"
    );
}
