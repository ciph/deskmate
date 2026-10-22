use super::*;
use serde_json::json;
use std::{
    io::{Cursor, Write},
    net::TcpListener,
    sync::atomic::AtomicU64,
};

fn directory(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}
fn scope() -> RefreshScope {
    RefreshScope {
        generation: 0,
        run_id: "run-a".into(),
        directory: directory("owned"),
        session_id: "s-owned".into(),
    }
}
fn connected() -> PermissionEvents {
    let events = PermissionEvents {
        default_directory: directory_key(&directory("owned")),
        ..PermissionEvents::default()
    };
    events.begin_connection(0).unwrap();
    events
}
fn event(kind: &str, id: &str, properties: Value) -> Value {
    json!({"directory":directory("owned"), "payload":{"type":kind,"id":id,"properties":properties}})
}
fn delta(id: &str, message: &str) -> Value {
    event(
        "message.part.delta",
        id,
        json!({"sessionID":"s-owned","messageID":message,"partID":"part-a","delta":"x"}),
    )
}
fn permission(id: &str, request: &str) -> Value {
    event(
        "permission.asked",
        id,
        json!({"id":request,"sessionID":"s-owned","permission":"webfetch","patterns":["https://example.test"],"metadata":{"url":"https://example.test"},"tool":{"messageID":"m-tool","callID":"call-a"}}),
    )
}
fn clear_initial(events: &PermissionEvents) {
    let first = events.changes(&scope()).unwrap();
    assert!(first.full && first.connected);
    events.ack(&scope(), first.ticket).unwrap();
}

#[test]
fn dirty_messages_coalesce_and_late_events_survive_ack() {
    let events = connected();
    clear_initial(&events);
    events.accept(0, delta("event-a", "m-a"));
    events.accept(0, delta("event-b", "m-a"));
    let before = events.changes(&scope()).unwrap();
    assert!(!before.full);
    assert_eq!(before.message_ids, ["m-a"]);
    events.accept(0, delta("event-c", "m-a"));
    events.accept(0, delta("event-d", "m-b"));
    events.ack(&scope(), before.ticket).unwrap();
    let after = events.changes(&scope()).unwrap();
    assert_eq!(after.message_ids, ["m-a", "m-b"]);
    events.ack(&scope(), after.ticket).unwrap();
    assert!(events.changes(&scope()).unwrap().message_ids.is_empty());
}

#[test]
fn late_full_invalidation_survives_ack() {
    let events = connected();
    clear_initial(&events);
    events.accept(
        0,
        event("session.idle", "event-a", json!({"sessionID":"s-owned"})),
    );
    let before = events.changes(&scope()).unwrap();
    events.accept(
        0,
        event("session.error", "event-b", json!({"sessionID":"s-owned"})),
    );
    events.ack(&scope(), before.ticket).unwrap();
    assert!(events.changes(&scope()).unwrap().full);
}

#[test]
fn routing_ignores_foreign_directory_session_generation_and_workspace() {
    let events = connected();
    clear_initial(&events);
    let mut foreign = delta("event-a", "m-a");
    foreign["directory"] = json!(directory("foreign"));
    events.accept(0, foreign);
    let mut foreign = delta("event-b", "m-a");
    foreign["payload"]["properties"]["sessionID"] = json!("s-foreign");
    events.accept(0, foreign);
    events.accept(1, delta("event-c", "m-a"));
    let mut foreign = delta("event-d", "m-a");
    foreign["workspace"] = json!("workspace-remote");
    events.accept(0, foreign);
    let changes = events.changes(&scope()).unwrap();
    assert!(!changes.full);
    assert!(changes.message_ids.is_empty());
}

#[test]
fn duplicates_are_bounded_and_sync_cannot_consume_normal_event_id() {
    let events = connected();
    clear_initial(&events);
    events.accept(0, event("sync", "event-a", json!({})));
    events.accept(0, delta("event-a", "m-a"));
    let first = events.changes(&scope()).unwrap();
    assert_eq!(first.message_ids, ["m-a"]);
    events.ack(&scope(), first.ticket).unwrap();
    events.accept(0, delta("event-a", "m-a"));
    assert!(events.changes(&scope()).unwrap().message_ids.is_empty());
    for n in 0..300 {
        events.accept(0, delta(&format!("event-{n}"), "m-a"));
    }
    let state = events.state.lock().unwrap();
    let active = state.active.as_ref().unwrap();
    assert_eq!(active.dirty.len(), 1);
    drop(state);
    // Evicted replay can cause harmless extra refresh, never loss of current changes.
    let pending = events.changes(&scope()).unwrap();
    events.ack(&scope(), pending.ticket).unwrap();
    events.accept(0, delta("event-a", "m-a"));
    assert_eq!(events.changes(&scope()).unwrap().message_ids, ["m-a"]);
}

#[test]
fn overflow_malformed_removed_and_permission_force_authoritative_refresh() {
    let events = connected();
    clear_initial(&events);
    for n in 0..65 {
        events.accept(0, delta(&format!("event-{n}"), &format!("m-{n}")));
    }
    let changes = events.changes(&scope()).unwrap();
    assert!(changes.full);
    assert_eq!(changes.message_ids.len(), 64);
    events.ack(&scope(), changes.ticket).unwrap();
    for (kind, properties) in [
        (
            "message.part.updated",
            json!({"sessionID":"s-owned","part":{}}),
        ),
        (
            "message.removed",
            json!({"sessionID":"s-owned","messageID":"m-a"}),
        ),
        (
            "message.part.removed",
            json!({"sessionID":"s-owned","messageID":"m-a"}),
        ),
        (
            "session.status",
            json!({"sessionID":"s-owned","status":{"type":"idle"}}),
        ),
        ("question.asked", json!({"sessionID":"s-owned"})),
    ] {
        events.accept(0, event(kind, kind, properties));
        let changes = events.changes(&scope()).unwrap();
        assert!(changes.full, "{kind}");
        events.ack(&scope(), changes.ticket).unwrap();
    }
    events.accept(0, permission("event-permission", "p-a"));
    let changes = events.changes(&scope()).unwrap();
    assert!(changes.full);
    assert_eq!(changes.message_ids, ["m-tool"]);
}

#[test]
fn complete_updates_use_parent_message_and_busy_is_not_full() {
    let events = connected();
    clear_initial(&events);
    events.accept(
        0,
        event(
            "message.updated",
            "event-a",
            json!({"sessionID":"s-owned","info":{"id":"m-a","sessionID":"s-owned"}}),
        ),
    );
    events.accept(0, event("message.part.updated", "event-b", json!({"sessionID":"s-owned","part":{"id":"part-b","messageID":"m-b","sessionID":"s-owned"}})));
    events.accept(
        0,
        event(
            "session.status",
            "event-c",
            json!({"sessionID":"s-owned","status":{"type":"busy"}}),
        ),
    );
    let changes = events.changes(&scope()).unwrap();
    assert!(!changes.full);
    assert_eq!(changes.message_ids, ["m-a", "m-b"]);
}

#[test]
fn permission_fallback_isolated_by_directory_session_and_request() {
    let events = connected();
    events.accept(0, permission("event-a", "p-shared"));
    let mut foreign = permission("event-b", "p-shared");
    foreign["directory"] = json!(directory("foreign"));
    events.accept(0, foreign);
    let mut foreign = permission("event-c", "p-shared");
    foreign["payload"]["properties"]["sessionID"] = json!("s-foreign");
    events.accept(0, foreign);
    assert_eq!(events.snapshot().unwrap().len(), 2);
    let owned = directory_key(&directory("owned")).unwrap();
    assert_eq!(
        events
            .snapshot_directory(&owned, Some("s-owned"))
            .unwrap()
            .len(),
        1
    );
    let mut reply = event(
        "permission.replied",
        "event-d",
        json!({"sessionID":"s-owned","requestID":"p-shared"}),
    );
    reply["directory"] = json!(directory("foreign"));
    events.accept(0, reply);
    assert_eq!(
        events
            .snapshot_directory(&owned, Some("s-owned"))
            .unwrap()
            .len(),
        1
    );
    events.accept(
        0,
        event(
            "permission.replied",
            "event-e",
            json!({"sessionID":"s-owned","requestID":"p-shared"}),
        ),
    );
    assert!(events.snapshot_directory(&owned, Some("s-owned")).is_err());
    assert_eq!(
        events
            .snapshot_directory(&owned, Some("s-foreign"))
            .unwrap()
            .len(),
        1
    );
    events.accept(0, permission("event-a", "p-shared"));
    assert!(
        events.snapshot_directory(&owned, Some("s-owned")).is_err(),
        "duplicate asked cannot reopen settled permission"
    );
}

#[test]
fn generation_change_rejects_cache_even_before_old_stream_disconnects() {
    let generation = Arc::new(AtomicU64::new(0));
    let current = generation.clone();
    let events = PermissionEvents {
        current_generation: Arc::new(move || Ok(current.load(Ordering::SeqCst))),
        default_directory: directory_key(&directory("owned")),
        ..PermissionEvents::default()
    };
    events.begin_connection(0).unwrap();
    clear_initial(&events);
    events.accept(0, permission("event-a", "p-a"));
    assert!(events.snapshot().is_ok());
    generation.store(1, Ordering::SeqCst);
    assert!(events.snapshot().is_err());
    events.accept(0, delta("event-b", "m-a"));
    let old = events.changes(&scope()).unwrap();
    assert!(!old.connected && old.full);
    assert!(events.begin_connection(0).is_err());
    events.begin_connection(1).unwrap();
    let next = RefreshScope {
        generation: 1,
        ..scope()
    };
    let changes = events.changes(&next).unwrap();
    assert!(changes.connected && changes.full);
    assert!(events.snapshot().is_err());
    events.ack(&scope(), u64::MAX).unwrap();
    assert!(events.changes(&next).unwrap().full);
}

#[test]
fn reconnect_and_run_switch_force_refresh_without_old_dirty_ids() {
    let events = connected();
    clear_initial(&events);
    events.accept(0, delta("event-a", "m-a"));
    let old = events.changes(&scope()).unwrap();
    let next = RefreshScope {
        run_id: "run-next".into(),
        ..scope()
    };
    let switched = events.changes(&next).unwrap();
    assert!(switched.full);
    assert!(switched.message_ids.is_empty());
    events.ack(&scope(), old.ticket).unwrap();
    assert!(events.changes(&next).unwrap().full);
    events.ack(&next, switched.ticket).unwrap();
    events.begin_connection(0).unwrap();
    assert!(events.changes(&next).unwrap().full);
    events.deactivate().unwrap();
    assert!(events.state.lock().unwrap().active.is_none());
    assert!(events.changes(&next).unwrap().full);
    events.stop();
    assert!(!events.changes(&next).unwrap().connected);
}

#[test]
fn sse_frames_handle_comments_multiline_and_malformed_data_fail_closed() {
    let events = connected();
    clear_initial(&events);
    events.accept(0, permission("event-a", "p-a"));
    let pending = events.changes(&scope()).unwrap();
    events.ack(&scope(), pending.ticket).unwrap();
    let json = serde_json::to_string_pretty(&delta("event-b", "m-a")).unwrap();
    let frame = format!(
        ": keepalive\r\n{}\r\n\r\n",
        json.lines()
            .map(|line| format!("data: {line}"))
            .collect::<Vec<_>>()
            .join("\r\n")
    );
    events.read_frames(Cursor::new(frame), 0).unwrap();
    assert_eq!(events.changes(&scope()).unwrap().message_ids, ["m-a"]);
    assert_eq!(
        events.read_frames(Cursor::new("data: {bad json}\n\n"), 0),
        Err("permission_invalid_event".into())
    );
    assert!(events.changes(&scope()).unwrap().full);
    assert!(events.snapshot().is_err());
    events.begin_connection(0).unwrap();
    events.accept(0, permission("event-c", "p-b"));
    assert!(events.snapshot().is_ok());
}

#[test]
fn stream_uses_single_global_endpoint_and_parses_wire_envelope() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let body = format!("data: {}\n\n", permission("event-a", "p-a"));
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 4096];
        let count = stream.read(&mut request).unwrap();
        assert!(
            String::from_utf8_lossy(&request[..count]).starts_with("GET /global/event HTTP/1.1")
        );
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    let events = connected();
    events.read(&base).unwrap();
    server.join().unwrap();
    assert_eq!(events.snapshot().unwrap()[0].id, "p-a");
    events.state.lock().unwrap().disconnect();
    assert!(events.snapshot().is_err());
}

#[test]
fn pending_permission_cache_overflow_fails_closed_until_reconnect() {
    let events = connected();
    for n in 0..=MAX_PENDING_PERMISSIONS {
        events.accept(0, permission(&format!("event-{n}"), &format!("p-{n}")));
    }
    assert!(events.snapshot().is_err());
    let state = events.state.lock().unwrap();
    assert!(state.requests.is_empty());
    assert!(state.seen_permissions.len() <= MAX_SEEN_PERMISSIONS);
    drop(state);
    events.begin_connection(0).unwrap();
    events.accept(0, permission("event-fresh", "p-fresh"));
    assert_eq!(events.snapshot().unwrap().len(), 1);
}

#[test]
fn missing_directory_and_contradictory_session_fail_to_full_refresh() {
    let events = connected();
    clear_initial(&events);
    let mut malformed = delta("event-a", "m-a");
    malformed.as_object_mut().unwrap().remove("directory");
    events.accept(0, malformed);
    let changed = events.changes(&scope()).unwrap();
    assert!(changed.full);
    events.ack(&scope(), changed.ticket).unwrap();
    events.accept(
        0,
        event(
            "message.updated",
            "event-b",
            json!({"sessionID":"s-owned","info":{"id":"m-a","sessionID":"s-other"}}),
        ),
    );
    assert!(events.changes(&scope()).unwrap().full);
}

#[test]
fn oversized_frame_is_bounded_and_requires_full_refresh() {
    let events = connected();
    clear_initial(&events);
    let frame = vec![b'x'; MAX_FRAME_BYTES + 2];
    assert_eq!(
        events.read_frames(Cursor::new(frame), 0),
        Err("permission_event_too_large".into())
    );
    assert!(events.changes(&scope()).unwrap().full);
}

#[test]
fn instance_and_global_disposal_revoke_scoped_permissions_and_refresh() {
    let events = connected();
    clear_initial(&events);
    events.accept(0, permission("event-a", "p-a"));
    let mut foreign = permission("event-b", "p-b");
    foreign["directory"] = json!(directory("foreign"));
    events.accept(0, foreign);
    let pending = events.changes(&scope()).unwrap();
    events.ack(&scope(), pending.ticket).unwrap();
    let mut disposed = event(
        "server.instance.disposed",
        "event-c",
        json!({"directory":directory("foreign")}),
    );
    disposed["directory"] = json!(directory("foreign"));
    events.accept(0, disposed);
    assert!(!events.changes(&scope()).unwrap().full);
    assert_eq!(events.snapshot().unwrap().len(), 1);
    assert!(events
        .snapshot_directory(&directory_key(&directory("foreign")).unwrap(), None)
        .is_err());
    events.accept(
        0,
        event(
            "server.instance.disposed",
            "event-d",
            json!({"directory":directory("owned")}),
        ),
    );
    assert!(events.changes(&scope()).unwrap().full);
    assert!(events.snapshot().is_err());
    events.accept(0, permission("event-e", "p-new"));
    assert_eq!(events.snapshot().unwrap().len(), 1);
    let pending = events.changes(&scope()).unwrap();
    events.ack(&scope(), pending.ticket).unwrap();
    events.accept(
        0,
        json!({"directory":"global","payload":{"type":"global.disposed","properties":{}}}),
    );
    assert!(events.changes(&scope()).unwrap().full);
    assert!(events.snapshot().is_err());
    assert!(events.changes(&scope()).unwrap().connected);
}

#[test]
#[ignore = "requires an isolated live OpenCode instance"]
fn live_web_permission_without_timeout() {
    let base = std::env::var("YUME_PERMISSION_TEST_BASE").expect("isolated engine URL");
    assert!(base.starts_with("http://127.0.0.1:"));
    let ready = std::env::var("YUME_PERMISSION_TEST_READY").expect("ready file");
    let events = PermissionEvents::start(base.clone());
    let deadline = std::time::Instant::now() + Duration::from_secs(40);
    while !events.state.lock().expect("lock").connected {
        assert!(
            std::time::Instant::now() < deadline,
            "stream did not connect"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    std::fs::write(ready, "ready").expect("write readiness");
    let requests = loop {
        if let Ok(requests) = super::super::runtime::pending_with_events(&base, &events) {
            if !requests.is_empty() {
                break requests;
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "web permission did not arrive"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].permission, "webfetch");
    assert!(requests[0].metadata.get("timeout").is_none());
    let session = requests[0].session_id.clone();
    let waiting = super::super::runtime::resolve_requests(
        &base,
        &session,
        &super::super::ToolPermissions {
            web: super::super::Mode::Deny,
            ..Default::default()
        },
        requests,
    )
    .expect("host rejects web request");
    assert!(waiting.is_empty());
    events.stop();
}

#[test]
fn relative_directory_is_rejected_before_canonicalization() {
    let events = connected();
    clear_initial(&events);
    let mut malformed = permission("event-relative", "p-relative");
    malformed["directory"] = json!(".");
    events.accept(0, malformed);
    assert!(events.changes(&scope()).unwrap().full);
    let state = events.state.lock().unwrap();
    assert!(state.requests.is_empty());
    assert!(state.permissions_invalid);
}

#[test]
fn reply_before_asked_cannot_resurrect_request_or_cross_scope() {
    let events = connected();
    events.accept(
        0,
        event(
            "permission.replied",
            "event-reply",
            json!({"sessionID":"s-owned","requestID":"p-shared"}),
        ),
    );
    events.accept(0, permission("event-late-asked", "p-shared"));
    let owned = directory_key(&directory("owned")).unwrap();
    assert!(events.snapshot_directory(&owned, Some("s-owned")).is_err());
    let mut foreign = permission("event-other-directory", "p-shared");
    foreign["directory"] = json!(directory("foreign"));
    events.accept(0, foreign);
    assert_eq!(
        events
            .snapshot_directory(
                &directory_key(&directory("foreign")).unwrap(),
                Some("s-owned")
            )
            .unwrap()
            .len(),
        1
    );
    let mut foreign = permission("event-other-session", "p-shared");
    foreign["payload"]["properties"]["sessionID"] = json!("s-other");
    events.accept(0, foreign);
    assert_eq!(
        events
            .snapshot_directory(&owned, Some("s-other"))
            .unwrap()
            .len(),
        1
    );
    events.begin_connection(0).unwrap();
    events.accept(0, permission("event-fresh", "p-shared"));
    assert_eq!(
        events
            .snapshot_directory(&owned, Some("s-owned"))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn settled_permission_bound_invalidates_instead_of_forgetting_late_replies() {
    let events = connected();
    for n in 0..=MAX_PENDING_PERMISSIONS {
        events.accept(
            0,
            event(
                "permission.replied",
                &format!("event-{n}"),
                json!({"sessionID":"s-owned","requestID":format!("p-{n}")}),
            ),
        );
    }
    let state = events.state.lock().unwrap();
    assert_eq!(state.settled.len(), MAX_PENDING_PERMISSIONS);
    assert!(state.permissions_invalid);
    drop(state);
    assert!(events.snapshot().is_err());
    events.begin_connection(0).unwrap();
    assert!(events.state.lock().unwrap().settled.is_empty());
}
