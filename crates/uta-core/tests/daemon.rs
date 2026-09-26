use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rusqlite::Connection;
use tempfile::TempDir;

#[cfg(unix)]
use uta_base::{IntegrationId, ProcessRole};
#[cfg(unix)]
use uta_store::{InstanceEnd, UnixMillis};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const EXIT_TIMEOUT: Duration = Duration::from_secs(15);
const STDERR_POLL_INTERVAL: Duration = Duration::from_millis(50);

struct Daemon {
    child: Child,
    stderr_lines: Vec<String>,
    stderr_rx: Receiver<String>,
    stderr_reader: Option<JoinHandle<()>>,
}

impl Daemon {
    fn spawn(home: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_uta-core"))
            .env("OPENALICE_HOME", home)
            .env("UTA_LOG", "info")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn uta-core");
        let stderr = child.stderr.take().expect("stderr was piped");
        let (stderr_tx, stderr_rx) = mpsc::channel();
        let stderr_reader = thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                match line {
                    Ok(line) => {
                        if stderr_tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            child,
            stderr_lines: Vec::new(),
            stderr_rx,
            stderr_reader: Some(stderr_reader),
        }
    }

    fn wait_ready(&mut self) {
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.collect_stderr();
                panic!("uta-core did not report readiness:\n{}", self.logs());
            }

            match self.stderr_rx.recv_timeout(remaining) {
                Ok(line) => {
                    let ready = line.contains("core ready");
                    self.stderr_lines.push(line);
                    if ready {
                        return;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    self.collect_stderr();
                    panic!("uta-core did not report readiness:\n{}", self.logs());
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.finish_stderr();
                    let status = self.child.try_wait().expect("check daemon status");
                    panic!(
                        "uta-core stderr closed before readiness (status {status:?}):\n{}",
                        self.logs()
                    );
                }
            }
        }
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> ExitStatus {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.child.try_wait().expect("check daemon status") {
                self.finish_stderr();
                return status;
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.collect_stderr();
                panic!("uta-core did not exit within {timeout:?}:\n{}", self.logs());
            }

            match self
                .stderr_rx
                .recv_timeout(remaining.min(STDERR_POLL_INTERVAL))
            {
                Ok(line) => self.stderr_lines.push(line),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    thread::sleep(remaining.min(STDERR_POLL_INTERVAL))
                }
            }
        }
    }

    fn force_kill(&mut self) -> ExitStatus {
        self.child.kill().expect("kill uta-core");
        self.wait_for_exit(EXIT_TIMEOUT)
    }

    #[cfg(unix)]
    fn send_signal(&mut self, signal: libc::c_int) {
        let pid = libc::pid_t::try_from(self.child.id()).expect("daemon PID fits pid_t");
        // SAFETY: `pid` is the live child process spawned by this test.
        let result = unsafe { libc::kill(pid, signal) };
        assert_eq!(
            result,
            0,
            "send signal {signal} to daemon PID {pid}: {}",
            std::io::Error::last_os_error()
        );
    }

    fn logs(&self) -> String {
        self.stderr_lines.join("\n")
    }

    fn collect_stderr(&mut self) {
        while let Ok(line) = self.stderr_rx.try_recv() {
            self.stderr_lines.push(line);
        }
    }

    fn finish_stderr(&mut self) {
        if let Some(reader) = self.stderr_reader.take()
            && reader.join().is_err()
        {
            self.stderr_lines
                .push("stderr reader thread panicked".to_owned());
        }
        self.collect_stderr();
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.finish_stderr();
    }
}

#[cfg(unix)]
struct SleepChild {
    child: Child,
}

#[cfg(unix)]
impl Drop for SleepChild {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn db_path(home: &Path) -> PathBuf {
    home.join("data").join("uta").join("uta.db")
}

fn instance_count(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT COUNT(*) FROM instances", [], |row| row.get(0))
        .expect("read instance count")
}

#[test]
fn second_instance_is_refused_while_first_holds_the_fence() {
    let home = TempDir::new().expect("create isolated state root");
    let mut first = Daemon::spawn(home.path());
    first.wait_ready();

    let mut second = Daemon::spawn(home.path());
    let status = second.wait_for_exit(EXIT_TIMEOUT);
    assert_eq!(
        status.code(),
        Some(10),
        "second instance must be refused by the fence:\n{}",
        second.logs()
    );
    assert!(
        second
            .logs()
            .contains("another UTA core instance holds the lock"),
        "second instance should report the held fence:\n{}",
        second.logs()
    );
    assert!(
        first
            .child
            .try_wait()
            .expect("check first daemon")
            .is_none(),
        "the first daemon must remain running when the second is refused"
    );

    let _ = first.force_kill();
}

#[cfg(unix)]
#[test]
fn sigterm_stops_cleanly_restarts_and_persists_the_end_anchor() {
    let home = TempDir::new().expect("create isolated state root");
    let mut first = Daemon::spawn(home.path());
    first.wait_ready();
    first.send_signal(libc::SIGTERM);
    let first_status = first.wait_for_exit(EXIT_TIMEOUT);
    assert_eq!(
        first_status.code(),
        Some(0),
        "SIGTERM should complete controlled stop:\n{}",
        first.logs()
    );

    let mut second = Daemon::spawn(home.path());
    second.wait_ready();
    assert!(
        second.logs().contains("previous instance stopped"),
        "restart should observe instance 1's stopped anchor:\n{}",
        second.logs()
    );
    second.send_signal(libc::SIGTERM);
    let second_status = second.wait_for_exit(EXIT_TIMEOUT);
    assert_eq!(
        second_status.code(),
        Some(0),
        "the restarted daemon should stop cleanly:\n{}",
        second.logs()
    );

    let db = db_path(home.path());
    let raw = Connection::open(&db).expect("open database for exact instance-1 row check");
    let instance_one_end: Option<i64> = raw
        .query_row(
            "SELECT ended_at_ms FROM instances WHERE instance_id = 1",
            [],
            |row| row.get(0),
        )
        .expect("read instance 1 end anchor");
    assert!(
        instance_one_end.is_some(),
        "instance 1's durable end anchor must remain present after instance 2 stops"
    );
    drop(raw);

    let mut store = uta_store::Store::open(&db).expect("open store after second daemon stopped");
    let later = store
        .begin_instance(UnixMillis::now().expect("read current time"))
        .expect("begin later inspection instance");
    let previous = store
        .previous_instance(&later)
        .expect("read preceding instance")
        .expect("the stopped daemon instance should precede the inspection instance");
    assert_eq!(previous.id.get(), 2);
    assert!(
        matches!(previous.end, InstanceEnd::Stopped { .. }),
        "previous instance should have a typed stopped end anchor: {:?}",
        previous.end
    );
    store
        .end_instance(later, UnixMillis::now().expect("read current time"))
        .expect("end inspection instance");
    store.close().expect("close inspection store");
}

#[cfg(unix)]
#[test]
fn sigkill_leaves_no_end_anchor_and_restart_reports_the_crash() {
    use std::os::unix::process::ExitStatusExt;

    let home = TempDir::new().expect("create isolated state root");
    let mut first = Daemon::spawn(home.path());
    first.wait_ready();
    first.send_signal(libc::SIGKILL);
    let killed_status = first.wait_for_exit(EXIT_TIMEOUT);
    assert_eq!(killed_status.signal(), Some(libc::SIGKILL));

    let mut restart = Daemon::spawn(home.path());
    restart.wait_ready();
    assert!(
        restart
            .logs()
            .contains("previous instance has no end anchor (crashed)"),
        "restart should report the prior crash:\n{}",
        restart.logs()
    );
    restart.send_signal(libc::SIGTERM);
    let restart_status = restart.wait_for_exit(EXIT_TIMEOUT);
    assert_eq!(
        restart_status.code(),
        Some(0),
        "the restart should run and stop normally:\n{}",
        restart.logs()
    );

    let raw = Connection::open(db_path(home.path())).expect("open database for crash row check");
    let instance_one_end: Option<i64> = raw
        .query_row(
            "SELECT ended_at_ms FROM instances WHERE instance_id = 1",
            [],
            |row| row.get(0),
        )
        .expect("read crashed instance end anchor");
    assert_eq!(
        instance_one_end, None,
        "SIGKILL must leave instance 1 without an end anchor"
    );
}

#[cfg(unix)]
#[test]
fn malformed_prior_process_row_fails_closed_and_stops_current_instance() {
    let home = TempDir::new().expect("create isolated state root");
    let mut first = Daemon::spawn(home.path());
    first.wait_ready();
    first.send_signal(libc::SIGTERM);
    let first_status = first.wait_for_exit(EXIT_TIMEOUT);
    assert_eq!(
        first_status.code(),
        Some(0),
        "initial daemon run should stop cleanly:\n{}",
        first.logs()
    );

    let db = db_path(home.path());
    let raw = Connection::open(&db).expect("open database to insert malformed process row");
    assert_eq!(
        instance_count(&raw),
        1,
        "initial daemon run should create exactly one instance"
    );
    let first_instance_id: i64 = raw
        .query_row(
            "SELECT instance_id FROM instances ORDER BY instance_id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("read completed instance ID");
    let inserted = raw
        .execute(
            "INSERT INTO processes (pid, start_time, instance_id, role_kind, role_id) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![42_i64, 1_i64, first_instance_id, "integration", ""],
        )
        .expect("insert process row with invalid integration ID");
    assert_eq!(inserted, 1, "malformed orphan row should be inserted");
    drop(raw);

    let mut second = Daemon::spawn(home.path());
    let second_status = second.wait_for_exit(EXIT_TIMEOUT);
    assert_eq!(
        second_status.code(),
        Some(1),
        "unreadable orphan rows should fail startup:\n{}",
        second.logs()
    );
    let logs = second.logs();
    assert!(
        !logs.contains("core ready"),
        "daemon must not report readiness after orphan-row parsing fails:\n{logs}"
    );
    assert!(
        logs.contains("cannot read the process table, orphans cannot be reclaimed")
            && logs.contains("process table row has an invalid id"),
        "stderr should identify the malformed process row as the reclamation failure:\n{logs}"
    );

    let raw = Connection::open(&db).expect("reopen database after failed startup");
    let (latest_instance_id, latest_end): (i64, Option<i64>) = raw
        .query_row(
            "SELECT instance_id, ended_at_ms FROM instances \
             ORDER BY instance_id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read newest instance end anchor");
    assert!(
        latest_instance_id > first_instance_id,
        "failed startup should have opened an instance after the malformed row's instance"
    );
    assert!(
        latest_end.is_some(),
        "controlled stop should persist an end anchor for the failed startup instance"
    );
}

#[cfg(unix)]
#[test]
fn startup_reclaims_orphan_process_and_clears_its_row() {
    use uta_proc::is_running;

    let home = TempDir::new().expect("create isolated state root");
    let db = db_path(home.path());
    std::fs::create_dir_all(db.parent().expect("database parent directory"))
        .expect("create database directory");

    let mut orphan = SleepChild {
        child: Command::new("sleep")
            .arg("60")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn live sleep child"),
    };
    let orphan_id = observe_until_running(orphan.child.id());
    assert!(
        is_running(orphan_id),
        "sleep child should be live before startup"
    );

    let mut store = uta_store::Store::open(&db).expect("open store to seed crash state");
    let crashed_instance = store
        .begin_instance(UnixMillis::now().expect("read current time"))
        .expect("begin seed instance");
    let role = ProcessRole::Integration(
        IntegrationId::parse("orphan-child").expect("valid integration id"),
    );
    store
        .transact(|tx| tx.processes().register(&crashed_instance, orphan_id, &role))
        .expect("register live child under the crashed instance")
        .into_inner();
    drop(crashed_instance);
    store
        .close()
        .expect("close seeded store without ending its instance");

    let mut daemon = Daemon::spawn(home.path());
    daemon.wait_ready();
    assert!(
        !is_running(orphan_id),
        "orphan process should be gone before core readiness"
    );
    assert!(
        daemon.logs().contains("orphan reclaimed"),
        "startup should log successful orphan reclamation:\n{}",
        daemon.logs()
    );
    assert!(
        orphan
            .child
            .try_wait()
            .expect("reap orphan status")
            .is_some(),
        "the reclaimed sleep child should be waitable"
    );

    daemon.send_signal(libc::SIGTERM);
    let status = daemon.wait_for_exit(EXIT_TIMEOUT);
    assert_eq!(
        status.code(),
        Some(0),
        "daemon should stop after orphan reclamation:\n{}",
        daemon.logs()
    );

    let mut store = uta_store::Store::open(&db).expect("open store to inspect reclaimed rows");
    let fresh = store
        .begin_instance(UnixMillis::now().expect("read current time"))
        .expect("begin fresh inspection instance");
    let other_rows = store
        .processes_of_other_instances(&fresh)
        .expect("read process rows from other instances");
    assert!(
        other_rows.is_empty(),
        "no processes from previous instances should remain: {other_rows:?}"
    );
    store
        .end_instance(fresh, UnixMillis::now().expect("read current time"))
        .expect("end inspection instance");
    store.close().expect("close inspection store");
}

#[cfg(unix)]
fn observe_until_running(pid: u32) -> uta_proc::OsProcessId {
    let deadline = Instant::now() + EXIT_TIMEOUT;
    loop {
        if let Some(process) = uta_proc::observe(pid) {
            return process;
        }
        assert!(
            Instant::now() < deadline,
            "OS did not report spawned sleep process {pid} as live"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn newer_database_format_is_refused_without_inserting_an_instance() {
    let home = TempDir::new().expect("create isolated state root");
    let mut normal = Daemon::spawn(home.path());
    normal.wait_ready();
    #[cfg(unix)]
    {
        normal.send_signal(libc::SIGTERM);
        let status = normal.wait_for_exit(EXIT_TIMEOUT);
        assert_eq!(
            status.code(),
            Some(0),
            "normal daemon run should stop cleanly:\n{}",
            normal.logs()
        );
    }
    #[cfg(not(unix))]
    {
        normal.force_kill();
    }

    let db = db_path(home.path());
    let raw = Connection::open(&db).expect("open database to set future format");
    let before = instance_count(&raw);
    assert_eq!(
        before, 1,
        "normal daemon run should create exactly one instance"
    );
    let changed = raw
        .execute(
            "UPDATE schema_meta SET format_version = 99 WHERE singleton = 1",
            [],
        )
        .expect("set unsupported future database format");
    assert_eq!(changed, 1, "schema metadata row should exist");
    drop(raw);

    let mut refused = Daemon::spawn(home.path());
    let status = refused.wait_for_exit(EXIT_TIMEOUT);
    assert_eq!(
        status.code(),
        Some(11),
        "future format should be rejected with exit code 11:\n{}",
        refused.logs()
    );
    assert!(
        refused.logs().contains("format version 99"),
        "stderr should explain the unsupported format:\n{}",
        refused.logs()
    );
    assert!(
        !refused.logs().contains("core ready"),
        "a refused database must not report readiness"
    );

    let raw = Connection::open(&db).expect("reopen database after format refusal");
    assert_eq!(
        instance_count(&raw),
        before,
        "a failed format check must not insert an instance row"
    );
    let version: i64 = raw
        .query_row(
            "SELECT format_version FROM schema_meta WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .expect("read future format after refusal");
    assert_eq!(version, 99, "refused database format must remain unchanged");
}
