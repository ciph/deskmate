use super::*;
use std::{
    os::windows::process::CommandExt,
    process::{Child, Command, Stdio},
    sync::{Arc, Barrier},
};

fn record() -> LaunchRecord {
    LaunchRecord {
        version: 1,
        identifier: "test.app".into(),
        host: ProcessIdentity {
            pid: 100,
            created: 20,
            executable: "C:\\test\\yume.exe".into(),
        },
        sidecar: ProcessIdentity {
            pid: 200,
            created: 30,
            executable: "C:\\test\\opencode.exe".into(),
        },
    }
}

#[test]
fn cleanup_requires_proven_dead_host_and_exact_child_identity() {
    let record = record();
    let decide =
        |host, child| decision(&record, "test.app", &record.host.executable, &host, &child);
    assert_eq!(
        decide(
            ProcessState::Missing,
            ProcessState::Running(record.sidecar.clone())
        ),
        Decision::Terminate
    );
    assert_eq!(
        decide(
            ProcessState::Exited {
                pid: record.host.pid,
                created: record.host.created
            },
            ProcessState::Running(record.sidecar.clone())
        ),
        Decision::Terminate
    );
    assert_eq!(
        decide(
            ProcessState::Running(record.host.clone()),
            ProcessState::Running(record.sidecar.clone())
        ),
        Decision::Keep
    );
    assert_eq!(
        decide(
            ProcessState::Unknown,
            ProcessState::Running(record.sidecar.clone())
        ),
        Decision::Keep
    );
    assert_eq!(
        decide(ProcessState::Missing, ProcessState::Unknown),
        Decision::Keep
    );
    assert_eq!(
        decide(ProcessState::Missing, ProcessState::Missing),
        Decision::RemoveRecord
    );
    assert_eq!(
        decide(
            ProcessState::Missing,
            ProcessState::Exited {
                pid: record.sidecar.pid,
                created: record.sidecar.created
            }
        ),
        Decision::RemoveRecord
    );
}

#[test]
fn pid_reuse_application_mismatch_and_invalid_records_never_authorize_kill() {
    let mut record = record();
    let mut reused_host = record.host.clone();
    reused_host.created += 1;
    let mut reused_child = record.sidecar.clone();
    reused_child.created += 1;
    assert_eq!(
        decision(
            &record,
            "test.app",
            &record.host.executable,
            &ProcessState::Running(reused_host),
            &ProcessState::Running(record.sidecar.clone())
        ),
        Decision::Keep
    );
    assert_eq!(
        decision(
            &record,
            "test.app",
            &record.host.executable,
            &ProcessState::Missing,
            &ProcessState::Running(reused_child)
        ),
        Decision::Keep
    );
    let live_child = ProcessState::Running(record.sidecar.clone());
    assert_eq!(
        decision(
            &record,
            "test.app",
            &record.host.executable,
            &ProcessState::Exited {
                pid: record.host.pid,
                created: record.host.created + 1
            },
            &live_child
        ),
        Decision::Keep,
        "an exited process with a reused host PID is not the recorded host"
    );
    assert_eq!(
        decision(
            &record,
            "other.app",
            &record.host.executable,
            &ProcessState::Missing,
            &live_child
        ),
        Decision::Keep
    );
    assert_eq!(
        decision(
            &record,
            "test.app",
            "C:\\other\\yume.exe",
            &ProcessState::Missing,
            &live_child
        ),
        Decision::Keep
    );
    record.version = 2;
    assert_eq!(
        decision(
            &record,
            "test.app",
            &record.host.executable,
            &ProcessState::Missing,
            &live_child
        ),
        Decision::Keep
    );
    record.version = 1;
    record.sidecar.created = 1;
    assert_eq!(
        decision(
            &record,
            "test.app",
            &record.host.executable,
            &ProcessState::Missing,
            &live_child
        ),
        Decision::Keep
    );
}

#[test]
fn simultaneous_launches_have_one_primary_before_any_plugin_window_exists() {
    let identifier = format!("test.yume.a01.{}", uuid::Uuid::new_v4());
    let start = Arc::new(Barrier::new(16));
    let held = Arc::new(Barrier::new(16));
    let handles: Vec<_> = (0..16)
        .map(|_| {
            let (identifier, start, held) = (identifier.clone(), start.clone(), held.clone());
            std::thread::spawn(move || {
                start.wait();
                let guard = PrimaryGuard::acquire(&identifier).unwrap();
                let winner = guard.is_some();
                held.wait();
                // The winner must stay alive until all competitors have tried.
                drop(guard);
                winner
            })
        })
        .collect();
    assert_eq!(
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|winner| *winner)
            .count(),
        1
    );
    let primary = PrimaryGuard::acquire(&identifier).unwrap().unwrap();
    for _ in 0..10 {
        assert!(PrimaryGuard::acquire(&identifier).unwrap().is_none());
    }
    drop(primary);
    assert!(PrimaryGuard::acquire(&identifier).unwrap().is_some());
}

#[test]
fn old_plugin_mutex_without_a_window_does_not_prove_primary_ownership() {
    let identifier = format!("test.yume.a01.{}", uuid::Uuid::new_v4());
    let _old_plugin_mutex = named_handle(&format!("{identifier}-sim")).unwrap().unwrap();
    let new_guard = PrimaryGuard::acquire(&identifier).unwrap().unwrap();
    assert!(!new_guard.owns_plugin_window());
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct RecordedChildGuard(ProcessIdentity);
impl Drop for RecordedChildGuard {
    fn drop(&mut self) {
        let _ = crate::windows_process_tree::terminate_created(self.0.pid, self.0.created);
    }
}

fn sleeper() -> ChildGuard {
    ChildGuard(
        Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 60",
            ])
            .creation_flags(0x0800_0000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

// Invoked only by the isolated test below, never by the application. The parent
// kills this exact fixture PID to exercise a real abrupt host exit.
#[test]
#[ignore = "subprocess fixture for native ownership recovery"]
fn isolated_host_fixture() {
    let Ok(root) = std::env::var("YUME_A01_FIXTURE_ROOT") else {
        return;
    };
    let identifier = std::env::var("YUME_A01_FIXTURE_ID").unwrap();
    assert!(identifier.starts_with("test.yume.a01."));
    let root = PathBuf::from(root);
    let _primary = PrimaryGuard::acquire(&identifier).unwrap().unwrap();
    let child = sleeper();
    record_child(&identifier, &root, child.0.id()).unwrap();
    fs::write(root.join("ready"), child.0.id().to_string()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(45);
    while Instant::now() < deadline && !root.join("stop").exists() {
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn native_crash_reclaims_only_recorded_orphan_and_rejects_reused_identity() {
    let root = tempfile::tempdir().unwrap();
    let identifier = format!("test.yume.a01.{}", uuid::Uuid::new_v4());
    let mut sibling = sleeper();
    let mut host = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "windows_startup::tests::isolated_host_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("YUME_A01_FIXTURE_ROOT", root.path())
            .env("YUME_A01_FIXTURE_ID", &identifier)
            .creation_flags(0x0800_0000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    while !root.path().join("ready").is_file() {
        assert!(
            host.0.try_wait().unwrap().is_none(),
            "fixture host exited before readiness"
        );
        assert!(Instant::now() < deadline, "fixture host readiness timeout");
        std::thread::sleep(Duration::from_millis(20));
    }
    let child_pid: u32 = fs::read_to_string(root.path().join("ready"))
        .unwrap()
        .parse()
        .unwrap();
    let path = record_path(root.path(), child_pid);
    let record: LaunchRecord = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let _orphan_cleanup = RecordedChildGuard(record.sidecar.clone());
    assert!(PrimaryGuard::acquire(&identifier).unwrap().is_none());
    assert_eq!(
        decision(
            &record,
            &identifier,
            &record.host.executable,
            &inspect(host.0.id()),
            &inspect(child_pid)
        ),
        Decision::Keep
    );
    assert!(matches!(inspect(child_pid), ProcessState::Running(_)));
    host.0.kill().unwrap();
    host.0.wait().unwrap();
    drop(host);
    let primary = PrimaryGuard::acquire(&identifier).unwrap().unwrap();
    // A recorded PID without the matching birth time is never enough.
    let mut stale = LaunchRecord {
        sidecar: record.sidecar.clone(),
        host: record.host.clone(),
        identifier: record.identifier.clone(),
        version: record.version,
    };
    stale.sidecar.created += 1;
    fs::write(&path, serde_json::to_vec(&stale).unwrap()).unwrap();
    assert_eq!(primary.cleanup(root.path()).unwrap(), 0);
    assert!(matches!(inspect(child_pid), ProcessState::Running(_)));
    assert!(
        crate::windows_process_tree::terminate_created(child_pid, stale.sidecar.created).is_err()
    );
    assert!(matches!(inspect(child_pid), ProcessState::Running(_)));
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    let host_state = inspect(record.host.pid);
    let child_state = inspect(child_pid);
    let ProcessState::Running(current_host) = inspect(std::process::id()) else {
        panic!("current host identity unavailable");
    };
    assert_eq!(
        decision(&record, &identifier, &current_host.executable, &host_state, &child_state),
        Decision::Terminate,
        "orphan evidence: host={host_state:?}, child={child_state:?}, recorded_host={:?}, current_host={current_host:?}",
        record.host
    );
    assert_eq!(primary.cleanup(root.path()).unwrap(), 1);
    assert!(!matches!(inspect(child_pid), ProcessState::Running(_)));
    assert!(!path.exists());
    assert!(
        sibling.0.try_wait().unwrap().is_none(),
        "unrecorded sibling must survive"
    );
}
