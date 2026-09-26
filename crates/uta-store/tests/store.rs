use std::time::Duration;

use uta_base::{IntegrationId, ProcessRole};
use uta_store::{InstanceEnd, OpenError, Store, TxError, UnixMillis, FORMAT_VERSION};

fn db(dir: &tempfile::TempDir) -> std::path::PathBuf {
    dir.path().join("uta.db")
}

/// A row written by one instance is an orphan candidate for the next, and is
/// cleared only with the OS's exit confirmation (design §7.2 step 1).
#[cfg(unix)]
#[tokio::test]
async fn process_rows_of_an_earlier_instance_are_cleared_only_after_os_exit() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
    let process = uta_proc::observe(child.id()).expect("live child is observable");
    let role = ProcessRole::Integration(IntegrationId::parse("fixture").unwrap());

    let mut store = Store::open(&db(&dir)).unwrap();
    let first = store
        .transact(|tx| tx.instances().begin(UnixMillis::now()))
        .unwrap()
        .into_inner();
    store
        .transact(|tx| tx.processes().register(&first, process, &role))
        .unwrap()
        .into_inner();
    assert!(
        store.processes_of_other_instances(&first).unwrap().is_empty(),
        "an instance's own rows are not orphans"
    );
    drop(first); // crash: no end anchor
    store.close().unwrap();

    let mut store = Store::open(&db(&dir)).unwrap();
    let second = store
        .transact(|tx| tx.instances().begin(UnixMillis::now()))
        .unwrap()
        .into_inner();
    let orphans = store.processes_of_other_instances(&second).unwrap();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].process, process);
    assert_eq!(orphans[0].role, role);

    let exited = uta_proc::reclaim(process, Duration::from_millis(200)).await.unwrap();
    assert!(!uta_proc::is_running(process));
    let _ = child.wait();
    store
        .transact(|tx| tx.processes().clear(exited))
        .unwrap()
        .into_inner();
    assert!(store.processes_of_other_instances(&second).unwrap().is_empty());
}

#[test]
fn instances_increment_and_the_end_anchor_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();

    let mut store = Store::open(&db(&dir)).unwrap();
    let first = store
        .transact(|tx| tx.instances().begin(UnixMillis::now()))
        .unwrap()
        .into_inner();
    assert_eq!(first.id().get(), 1);
    let stopped_at = UnixMillis::now();
    store
        .transact(|tx| tx.instances().end(first, stopped_at))
        .unwrap()
        .into_inner();
    store.close().unwrap();

    let mut store = Store::open(&db(&dir)).unwrap();
    let second = store
        .transact(|tx| tx.instances().begin(UnixMillis::now()))
        .unwrap()
        .into_inner();
    assert_eq!(second.id().get(), 2);
    let previous = store.previous_instance(&second).unwrap().unwrap();
    assert_eq!(previous.id.get(), 1);
    assert_eq!(previous.end, InstanceEnd::Stopped { at: stopped_at });
    store.close().unwrap();

    // A third instance after a "crash" of the second (no end anchor).
    let mut store = Store::open(&db(&dir)).unwrap();
    let third = store
        .transact(|tx| tx.instances().begin(UnixMillis::now()))
        .unwrap()
        .into_inner();
    assert_eq!(third.id().get(), 3);
    assert_eq!(
        store.previous_instance(&third).unwrap().unwrap().end,
        InstanceEnd::NoEndAnchor
    );
}

#[test]
fn an_aborted_transaction_leaves_no_trace() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&db(&dir)).unwrap();

    let aborted = store.transact(|tx| {
        let _ = tx.instances().begin(UnixMillis::now())?;
        Err::<(), _>(rusqlite_abort())
    });
    assert!(matches!(aborted, Err(TxError::Aborted(_))));
    store.close().unwrap();

    let mut store = Store::open(&db(&dir)).unwrap();
    let first = store
        .transact(|tx| tx.instances().begin(UnixMillis::now()))
        .unwrap()
        .into_inner();
    assert_eq!(first.id().get(), 1, "the aborted instance row must not exist");
    assert!(store.previous_instance(&first).unwrap().is_none());
}

#[test]
fn a_file_from_a_newer_build_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    Store::open(&db(&dir)).unwrap().close().unwrap();
    let raw = rusqlite::Connection::open(db(&dir)).unwrap();
    raw.execute("UPDATE schema_meta SET format_version = 99", [])
        .unwrap();
    drop(raw);

    match Store::open(&db(&dir)) {
        Err(OpenError::FormatTooNew { found, supported }) => {
            assert_eq!((found, supported), (99, FORMAT_VERSION));
        }
        other => panic!("expected FormatTooNew, got {other:?}"),
    }
}

#[test]
fn a_fresh_file_is_in_wal_mode_at_the_current_format() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&db(&dir)).unwrap();
    assert_eq!(store.format_version().unwrap(), FORMAT_VERSION);
    let raw = rusqlite::Connection::open(db(&dir)).unwrap();
    let mode: String = raw
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

fn rusqlite_abort() -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}
