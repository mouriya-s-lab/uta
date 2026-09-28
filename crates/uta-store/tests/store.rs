use uta_store::{FORMAT_VERSION, InstanceEnd, OpenError, Store, TxError, UnixMillis};

fn db(dir: &tempfile::TempDir) -> std::path::PathBuf {
    dir.path().join("uta.db")
}

#[cfg(unix)]
struct SleepChild(std::process::Child);

#[cfg(unix)]
impl Drop for SleepChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A row written by one instance is an orphan candidate for the next, and is
/// cleared only with the OS's exit confirmation (design core/core-process/design.md §4.7.3 step 1).
#[cfg(unix)]
#[tokio::test]
async fn process_rows_of_an_earlier_instance_are_cleared_only_after_os_exit() {
    use std::time::Duration;
    use uta_base::{IntegrationId, ProcessRole};

    let dir = tempfile::tempdir().unwrap();
    let child = SleepChild(
        std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap(),
    );
    let process = uta_proc::observe(child.0.id()).expect("live child is observable");
    let role = ProcessRole::Integration(IntegrationId::parse("fixture").unwrap());

    let mut store = Store::open(&db(&dir)).unwrap();
    let first = store.begin_instance(UnixMillis::now().unwrap()).unwrap();
    store
        .transact(|tx| tx.processes().register(&first, process, &role))
        .unwrap()
        .into_inner();
    assert!(
        store
            .processes_of_other_instances(&first)
            .unwrap()
            .is_empty(),
        "an instance's own rows are not orphans"
    );
    drop(first); // crash: no end anchor
    store.close().unwrap();
    let mut store = Store::open(&db(&dir)).unwrap();

    let second = store.begin_instance(UnixMillis::now().unwrap()).unwrap();
    let orphans = store.processes_of_other_instances(&second).unwrap();
    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].process, process);
    assert_eq!(orphans[0].role, role);
    assert!(uta_proc::is_running(process));
    let exited = uta_proc::reclaim(process, Duration::from_millis(200))
        .await
        .unwrap();
    assert!(!uta_proc::is_running(process));
    store
        .transact(|tx| tx.processes().clear(exited))
        .unwrap()
        .into_inner();
    assert!(
        store
            .processes_of_other_instances(&second)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn instances_increment_and_the_end_anchor_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();

    let mut store = Store::open(&db(&dir)).unwrap();
    let first = store.begin_instance(UnixMillis::now().unwrap()).unwrap();
    assert_eq!(first.id().get(), 1);
    let stopped_at = UnixMillis::now().unwrap();
    store.end_instance(first, stopped_at).unwrap();
    store.close().unwrap();

    let mut store = Store::open(&db(&dir)).unwrap();
    let second = store.begin_instance(UnixMillis::now().unwrap()).unwrap();
    assert_eq!(second.id().get(), 2);
    let previous = store.previous_instance(&second).unwrap().unwrap();
    assert_eq!(previous.id.get(), 1);
    assert_eq!(previous.end, InstanceEnd::Stopped { at: stopped_at });
    store.close().unwrap();

    // A third instance after a "crash" of the second (no end anchor).
    let mut store = Store::open(&db(&dir)).unwrap();
    let third = store.begin_instance(UnixMillis::now().unwrap()).unwrap();
    assert_eq!(third.id().get(), 3);
    assert_eq!(
        store.previous_instance(&third).unwrap().unwrap().end,
        InstanceEnd::NoEndAnchor
    );
}

#[test]
fn an_aborted_transaction_leaves_no_trace() {
    use uta_base::{IntegrationId, ProcessRole};

    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&db(&dir)).unwrap();
    let instance = store.begin_instance(UnixMillis::now().unwrap()).unwrap();
    let process = uta_proc::observe(std::process::id()).expect("test process is observable");
    let role = ProcessRole::Integration(IntegrationId::parse("fixture").unwrap());

    let aborted = store.transact(|tx| {
        tx.processes().register(&instance, process, &role)?;
        Err::<(), _>(rusqlite_abort())
    });
    assert!(matches!(aborted, Err(TxError::Aborted(_))));
    drop(instance);
    store.close().unwrap();

    let mut store = Store::open(&db(&dir)).unwrap();
    let next = store.begin_instance(UnixMillis::now().unwrap()).unwrap();
    assert!(
        store
            .processes_of_other_instances(&next)
            .unwrap()
            .is_empty(),
        "the aborted process registration must not exist"
    );
}

#[test]
fn a_file_from_a_newer_build_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    Store::open(&db(&dir)).unwrap().close().unwrap();
    let newer = FORMAT_VERSION + 1;
    let raw = rusqlite::Connection::open(db(&dir)).unwrap();
    raw.execute("UPDATE schema_meta SET format_version = ?1", [newer])
        .unwrap();
    drop(raw);

    match Store::open(&db(&dir)) {
        Err(OpenError::FormatTooNew { found, supported }) => {
            assert_eq!((found, supported), (newer, FORMAT_VERSION));
        }
        other => panic!("expected FormatTooNew, got {other:?}"),
    }
}

#[test]
fn unreadable_format_versions_are_refused() {
    for (case, mutation) in [
        ("missing meta row", "DELETE FROM schema_meta"),
        (
            "negative version",
            "UPDATE schema_meta SET format_version = -1",
        ),
        (
            "version above u32::MAX",
            "UPDATE schema_meta SET format_version = 4294967296",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        Store::open(&db(&dir)).unwrap().close().unwrap();
        let raw = rusqlite::Connection::open(db(&dir)).unwrap();
        raw.execute(mutation, []).unwrap();
        drop(raw);

        assert!(
            matches!(Store::open(&db(&dir)), Err(OpenError::FormatUnreadable(_))),
            "{case} should be rejected as an unreadable format"
        );
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
