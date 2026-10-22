use super::*;
use crate::worklog::{
    bridge::auth, contract::DateQuery, repository::Repository, storage::WorklogStore,
};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

const AUTH: &str = "Basic b3BlbmNvZGU6c3ludGhldGljLXRlc3Q=";

struct Server {
    endpoint: String,
    stop: Arc<AtomicBool>,
    calls: Arc<Mutex<Vec<(String, bool)>>>,
    user: Arc<Mutex<Value>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn start(reported_directory: Option<String>, pause: Option<Arc<std::sync::Barrier>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let user = Arc::new(Mutex::new(
            json!({"info":{"id":"msg_user","role":"user","sessionID":"ses_same"},"parts":[{"type":"text","text":"现在生成今天的工作日志，修改今天的工作日志，并且周五汇总周报"}]}),
        ));
        let (done, seen, native_user) = (stop.clone(), calls.clone(), user.clone());
        let thread = std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(error) => panic!("fixture accept: {error}"),
                };
                // Windows accepted sockets inherit the listener's nonblocking mode.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                while !bytes.ends_with(b"\r\n\r\n") {
                    let mut b = [0];
                    stream.read_exact(&mut b).unwrap();
                    bytes.push(b[0]);
                    assert!(bytes.len() < 8192);
                }
                let header = String::from_utf8(bytes).unwrap();
                let target = header
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap();
                let url = url::Url::parse(&format!("http://localhost{target}")).unwrap();
                let authorized = header.lines().any(|line| {
                    line.split_once(':').is_some_and(|(name, value)| {
                        name.eq_ignore_ascii_case("authorization") && value.trim() == AUTH
                    })
                });
                seen.lock().unwrap().push((target.into(), authorized));
                let directory = url
                    .query_pairs()
                    .find(|(key, _)| key == "directory")
                    .map(|(_, value)| value.into_owned())
                    .unwrap_or_default();
                let body = if !authorized { json!({"error":"unauthorized"}) }
                else if url.path() == "/session/ses_same" {
                    json!({"id":"ses_same","directory":reported_directory.as_ref().unwrap_or(&directory),"title":"fixture","time":{"created":1,"updated":1}})
                } else if url.path() == "/session/ses_same/message/msg_user" {
                    native_user.lock().unwrap().clone()
                } else {
                    assert_eq!(url.path(), "/session/ses_same/message/msg_assistant", "must not fetch unrelated message/attachment history");
                    if let Some(pause) = &pause { pause.wait(); pause.wait(); }
                    json!({"info":{"id":"msg_assistant","role":"assistant","sessionID":"ses_same","parentID":"msg_user"},"parts": (["record","query","update","generate_report","schedule_report"].map(|action| json!({"type":"tool","tool":format!("worklog_{action}"),"callID":"call_same"})))})
                }.to_string();
                let status = if authorized {
                    "200 OK"
                } else {
                    "401 Unauthorized"
                };
                write!(stream,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        });
        Self {
            endpoint,
            stop,
            calls,
            user,
            thread: Some(thread),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Err(error) = self.thread.take().unwrap().join() {
            // Preserve the original test failure instead of aborting on a second panic.
            if !std::thread::panicking() {
                std::panic::resume_unwind(error);
            }
        }
    }
}
fn bridge(server: &Server) -> (tempfile::TempDir, WorklogBridge, Launch) {
    let root = tempfile::tempdir().unwrap();
    let bridge = WorklogBridge {
        root: Some(root.path().into()),
        launch: Mutex::new(None),
        grants: Mutex::new(super::super::Registrations::default()),
    };
    bridge
        .reset_launch(server.endpoint.clone(), AUTH.into())
        .unwrap();
    let launch = bridge.launch.lock().unwrap().clone().unwrap();
    (root, bridge, launch)
}
fn request(directory: &str, action: &str, args: Value) -> Request {
    Request {
        version: 1,
        request_id: uuid::Uuid::new_v4().to_string(),
        directory: directory.into(),
        session_id: "ses_same".into(),
        message_id: "msg_assistant".into(),
        call_id: "call_same".into(),
        action: action.into(),
        args,
    }
}
fn register(bridge: &WorklogBridge, directory: &str, text: &str) {
    bridge
        .register(
            directory.into(),
            "ses_same".into(),
            "msg_user".into(),
            text.into(),
        )
        .unwrap();
}
fn execute(
    bridge: &WorklogBridge,
    launch: &Launch,
    repo: &Repository,
    request: &Request,
) -> WorklogResult<Value> {
    process(bridge, launch, request, |grant, user| {
        super::super::dispatch::execute(repo, request, grant, user, "fixture/model")
    })
}
fn query(date: &str) -> DateQuery {
    DateQuery {
        start: date.into(),
        end: date.into(),
        project: None,
    }
}

#[test]
fn authenticated_rust_bridge_executes_all_five_tools_with_workspace() {
    let server = Server::start(None, None);
    let (_root, bridge, launch) = bridge(&server);
    let repo = Repository::new(WorklogStore::memory().unwrap());
    let directory = "/workspace/项目 space";
    let date = auth::grant("today").unwrap().received_date.to_string();
    register(
        &bridge,
        directory,
        "把今天完成登录联调记入日报；现在生成今天的日报、修改今天的日报，并且周五汇总周报",
    );
    let saved = execute(
        &bridge,
        &launch,
        &repo,
        &request(
            directory,
            "record",
            json!({"mode":"direct","businessDate":date,"text":"登录联调完成","status":"done"}),
        ),
    )
    .unwrap();
    let id = saved["receipt"]["entityId"].as_str().unwrap();
    execute(&bridge,&launch,&repo,&request(directory,"update",json!({"id":id,"expectedRevision":1,"businessDate":date,"text":"登录联调验收完成","status":"done"}))).unwrap();
    let found = execute(
        &bridge,
        &launch,
        &repo,
        &request(directory, "query", json!({"start":date})),
    )
    .unwrap();
    assert_eq!(found["entries"][0]["revision"], 2);
    execute(
        &bridge,
        &launch,
        &repo,
        &request(
            directory,
            "generate_report",
            json!({"kind":"daily","periodStart":date,"periodEnd":date}),
        ),
    )
    .unwrap();
    execute(
        &bridge,
        &launch,
        &repo,
        &request(
            directory,
            "schedule_report",
            json!({"kind":"weekly","weekdaySet":[5],"enabled":true}),
        ),
    )
    .unwrap();
    assert_eq!(repo.list_schedules().unwrap().len(), 1);
    assert_eq!(
        repo.query_entries(&query(&date)).unwrap()[0].original_text,
        "把今天完成登录联调记入日报；现在生成今天的日报、修改今天的日报，并且周五汇总周报"
    );
    let calls = server.calls.lock().unwrap();
    assert_eq!(calls.len(), 10);
    assert!(calls.iter().all(|(target, authorized)| *authorized
        && url::Url::parse(&format!("http://localhost{target}"))
            .unwrap()
            .query_pairs()
            .any(|(key, value)| key == "directory" && value == directory)));
}

#[test]
fn wrong_or_missing_credentials_never_reach_dispatch() {
    let server = Server::start(None, None);
    let (_root, bridge, mut launch) = bridge(&server);
    for header in ["", "Basic wrong"] {
        launch.auth_header = header.into();
        for action in [
            "record",
            "query",
            "update",
            "generate_report",
            "schedule_report",
        ] {
            let result = process(
                &bridge,
                &launch,
                &request("/workspace/a", action, json!({})),
                |_, _| panic!("unauthenticated dispatch"),
            );
            assert!(result.is_err());
        }
    }
    assert_eq!(server.calls.lock().unwrap().len(), 5);
    assert!(server
        .calls
        .lock()
        .unwrap()
        .iter()
        .all(|(_, authorized)| !authorized));
}

#[test]
fn equal_ids_in_two_workspaces_cannot_borrow_or_revoke_grants() {
    let server = Server::start(None, None);
    let (_root, bridge, launch) = bridge(&server);
    let repo = Repository::new(WorklogStore::memory().unwrap());
    register(&bridge, "/workspace/a", "把今天完成 A 记入日报");
    let date = auth::grant("today").unwrap().received_date.to_string();
    let a = request(
        "/workspace/a",
        "record",
        json!({"mode":"direct","businessDate":date,"text":"A 完成","status":"done"}),
    );
    let b = request(
        "/workspace/b",
        "record",
        json!({"mode":"direct","businessDate":date,"text":"B 完成","status":"done"}),
    );
    assert_eq!(
        execute(&bridge, &launch, &repo, &b).unwrap_err().code,
        "NEEDS_EXPLICIT_REQUEST"
    );
    register(&bridge, "/workspace/b", "把今天完成 B 记入日报");
    execute(&bridge, &launch, &repo, &a).unwrap();
    execute(&bridge, &launch, &repo, &b).unwrap();
    let entries = repo.query_entries(&query(&date)).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries
        .iter()
        .any(|entry| entry.text == "A 完成" && entry.original_text.contains("A")));
    assert!(entries
        .iter()
        .any(|entry| entry.text == "B 完成" && entry.original_text.contains("B")));
}

#[test]
fn returned_session_directory_must_match_even_with_same_ids() {
    let server = Server::start(Some("/workspace/other".into()), None);
    let (_root, bridge, launch) = bridge(&server);
    let result = process(
        &bridge,
        &launch,
        &request("/workspace/a", "query", json!({})),
        |_, _| panic!("wrong directory dispatch"),
    );
    assert_eq!(result.unwrap_err().code, "INVALID_CONTEXT");
    assert_eq!(server.calls.lock().unwrap().len(), 1);
}

#[test]
fn old_launch_is_rejected_before_http_and_during_parent_lookup() {
    let pause = Arc::new(std::sync::Barrier::new(2));
    let server = Server::start(None, Some(pause.clone()));
    let (_root, bridge, launch) = bridge(&server);
    register(&bridge, "/workspace/a", "查询今天的工作日志");
    let bridge = Arc::new(bridge);
    let worker_bridge = bridge.clone();
    let old = launch.clone();
    let worker = std::thread::spawn(move || {
        process(
            &worker_bridge,
            &old,
            &request("/workspace/a", "query", json!({})),
            |_, _| panic!("stale dispatch"),
        )
    });
    pause.wait();
    bridge
        .reset_launch(server.endpoint.clone(), AUTH.into())
        .unwrap();
    pause.wait();
    assert_eq!(worker.join().unwrap().unwrap_err().code, "STALE_CONTEXT");
    let calls = server.calls.lock().unwrap().len();
    assert_eq!(
        process(
            &bridge,
            &launch,
            &request("/workspace/a", "query", json!({})),
            |_, _| panic!("old dispatch")
        )
        .unwrap_err()
        .code,
        "STALE_CONTEXT"
    );
    assert_eq!(server.calls.lock().unwrap().len(), calls);
    assert!(bridge.grants.lock().unwrap().active.is_empty());
}

#[test]
fn native_pending_requires_real_user_and_keeps_query_state_on_duplicate_registration() {
    let server = Server::start(None, None);
    let (_root, bridge, launch) = bridge(&server);
    let repo = Repository::new(WorklogStore::memory().unwrap());
    let date = auth::grant("today").unwrap().received_date.to_string();
    let directory = "/workspace/a";
    // The same host entry is used before automatic settings are consulted.
    bridge
        .register_native(
            &launch.directory,
            directory.into(),
            "ses_same".into(),
            "msg_user".into(),
        )
        .unwrap();
    let read = request(directory, "query", json!({"start":date}));
    execute(&bridge, &launch, &repo, &read).unwrap();
    let key = (directory.into(), "ses_same".into(), "msg_user".into());
    bridge.grants.lock().unwrap().active[&key]
        .grant
        .as_ref()
        .unwrap()
        .query_failed
        .set(true);
    bridge
        .register_native(
            &launch.directory,
            directory.into(),
            "ses_same".into(),
            "msg_user".into(),
        )
        .unwrap();
    let write = request(
        directory,
        "record",
        json!({"mode":"direct","businessDate":date,"text":"must not save","status":"done"}),
    );
    assert_eq!(
        execute(&bridge, &launch, &repo, &write).unwrap_err().code,
        "QUERY_REQUIRED"
    );
    assert!(repo.query_entries(&query(&date)).unwrap().is_empty());
}

#[test]
fn native_pending_rejects_spoofed_or_synthetic_parent_sources() {
    for info in [
        json!({"id":"msg_other","role":"user","sessionID":"ses_same"}),
        json!({"id":"msg_user","role":"assistant","sessionID":"ses_same"}),
        json!({"id":"msg_user","role":"user","sessionID":"ses_other"}),
    ] {
        let server = Server::start(None, None);
        server.user.lock().unwrap()["info"] = info;
        let (_root, bridge, launch) = bridge(&server);
        bridge
            .register_native(
                &launch.directory,
                "/workspace/a".into(),
                "ses_same".into(),
                "msg_user".into(),
            )
            .unwrap();
        assert_eq!(
            process(
                &bridge,
                &launch,
                &request("/workspace/a", "query", json!({})),
                |_, _| panic!("spoofed source")
            )
            .unwrap_err()
            .code,
            "INVALID_CONTEXT"
        );
    }
    let server = Server::start(None, None);
    server.user.lock().unwrap()["parts"] =
        json!([{"type":"text","text":"生成今天的工作日志","synthetic":true}]);
    let (_root, bridge, launch) = bridge(&server);
    bridge
        .register_native(
            &launch.directory,
            "/workspace/a".into(),
            "ses_same".into(),
            "msg_user".into(),
        )
        .unwrap();
    assert_eq!(
        process(
            &bridge,
            &launch,
            &request("/workspace/a", "query", json!({})),
            |_, _| panic!("synthetic source")
        )
        .unwrap_err()
        .code,
        "INVALID_CONTEXT"
    );
}

#[test]
fn duplicate_native_registration_cannot_revive_revoked_expired_or_old_launch_sources() {
    let server = Server::start(None, None);
    let (_root, bridge, launch) = bridge(&server);
    let native = |message: &str| {
        bridge.register_native(
            &launch.directory,
            "/workspace/a".into(),
            "ses_same".into(),
            message.into(),
        )
    };
    native("msg_user").unwrap();
    native("msg_later").unwrap();
    native("msg_user").unwrap();
    assert!(!bridge.grants.lock().unwrap().active.contains_key(&(
        "/workspace/a".into(),
        "ses_same".into(),
        "msg_user".into()
    )));
    let key = ("/workspace/a".into(), "ses_same".into(), "msg_later".into());
    bridge
        .grants
        .lock()
        .unwrap()
        .active
        .get_mut(&key)
        .unwrap()
        .created = std::time::Instant::now() - Duration::from_secs(1801);
    native("msg_later").unwrap();
    assert!(
        bridge.grants.lock().unwrap().active[&key]
            .created
            .elapsed()
            .as_secs()
            >= 1800
    );
    bridge
        .reset_launch(server.endpoint.clone(), AUTH.into())
        .unwrap();
    assert_eq!(native("msg_new").unwrap_err().code, "STALE_CONTEXT");
    assert!(bridge.grants.lock().unwrap().active.is_empty());
}

#[test]
fn native_pending_authorizes_all_five_tools_from_verified_parent() {
    for action in [
        "record",
        "query",
        "update",
        "generate_report",
        "schedule_report",
    ] {
        let server = Server::start(None, None);
        let (_root, bridge, launch) = bridge(&server);
        bridge
            .register_native(
                &launch.directory,
                "/workspace/a".into(),
                "ses_same".into(),
                "msg_user".into(),
            )
            .unwrap();
        process(
            &bridge,
            &launch,
            &request("/workspace/a", action, json!({})),
            |grant, user| {
                auth::authorize(grant, action)?;
                assert_eq!(user, "msg_user");
                assert_eq!(
                    grant.original,
                    server.user.lock().unwrap()["parts"][0]["text"]
                        .as_str()
                        .unwrap()
                );
                Ok(json!({"verified":true}))
            },
        )
        .unwrap();
        assert_eq!(server.calls.lock().unwrap().len(), 3);
    }
}

#[test]
fn windows_directory_aliases_share_the_host_grant_identity() {
    let server = Server::start(Some("C:/Workspace/Project/".into()), None);
    let (_root, bridge, launch) = bridge(&server);
    register(&bridge, "C:/workspace/project", "查询今天的工作日志");
    process(
        &bridge,
        &launch,
        &request("C:/WORKSPACE/Project/./", "query", json!({})),
        |_, _| Ok(json!({"verified":true})),
    )
    .unwrap();
}

#[test]
fn pending_native_turn_keeps_registration_business_date_across_0300() {
    let server = Server::start(None, None);
    let (_root, bridge, launch) = bridge(&server);
    let repo = Repository::new(WorklogStore::memory().unwrap());
    bridge
        .register_native(
            &launch.directory,
            "/workspace/a".into(),
            "ses_same".into(),
            "msg_user".into(),
        )
        .unwrap();
    // Simulate registration before the business-day boundary, execution after it.
    let source_date = auth::grant("today")
        .unwrap()
        .received_date
        .pred_opt()
        .unwrap();
    bridge
        .grants
        .lock()
        .unwrap()
        .active
        .get_mut(&("/workspace/a".into(), "ses_same".into(), "msg_user".into()))
        .unwrap()
        .received_date = source_date;
    execute(&bridge,&launch,&repo,&request("/workspace/a","generate_report",json!({"kind":"daily","periodStart":source_date.to_string(),"periodEnd":source_date.to_string()}))).unwrap();
}

#[test]
fn failed_launch_setup_revokes_old_grants_before_validation_or_filesystem_work() {
    for invalid_endpoint in [false, true] {
        let server = Server::start(None, None);
        let (root, mut bridge, old) = bridge(&server);
        let repo = Repository::new(WorklogStore::memory().unwrap());
        let date = auth::grant("today").unwrap().received_date.to_string();
        register(&bridge, "/workspace/a", "把今天完成登录联调记入日报");
        if !invalid_endpoint {
            let blocked = root.path().join("not-a-directory");
            std::fs::write(&blocked, b"fixture").unwrap();
            bridge.root = Some(blocked);
        }
        let endpoint = if invalid_endpoint {
            "https://example.invalid".into()
        } else {
            server.endpoint.clone()
        };
        assert!(bridge.reset_launch(endpoint, AUTH.into()).is_err());
        assert!(bridge.launch.lock().unwrap().is_none());
        assert!(bridge.grants.lock().unwrap().active.is_empty());
        assert!(bridge.grants.lock().unwrap().seen.is_empty());
        assert_eq!(bridge.launch_id().unwrap_err().code, "BRIDGE_UNAVAILABLE");
        assert_eq!(
            bridge
                .register(
                    "/workspace/a".into(),
                    "ses_same".into(),
                    "msg_new".into(),
                    "保存今天日报".into()
                )
                .unwrap_err()
                .code,
            "BRIDGE_UNAVAILABLE"
        );
        let write = request(
            "/workspace/a",
            "record",
            json!({"mode":"direct","businessDate":date,"text":"must not save","status":"done"}),
        );
        assert_eq!(
            execute(&bridge, &old, &repo, &write).unwrap_err().code,
            "STALE_CONTEXT"
        );
        assert!(repo.query_entries(&query(&date)).unwrap().is_empty());
        assert!(server.calls.lock().unwrap().is_empty());
        // Recovery publishes a fresh generation; a retired tool never becomes current again.
        bridge.root = Some(root.path().into());
        bridge
            .reset_launch(server.endpoint.clone(), AUTH.into())
            .unwrap();
        let recovered = bridge.launch.lock().unwrap().clone().unwrap();
        assert_ne!(old.directory, recovered.directory);
        register(&bridge, "/workspace/a", "查询今天的工作日志");
        execute(
            &bridge,
            &recovered,
            &repo,
            &request("/workspace/a", "query", json!({"start":date})),
        )
        .unwrap();
        assert_eq!(
            execute(&bridge, &old, &repo, &write).unwrap_err().code,
            "STALE_CONTEXT"
        );
        assert!(repo.query_entries(&query(&date)).unwrap().is_empty());
    }
}
