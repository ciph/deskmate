use super::opencode::{AgentEndpoint, OpenCodeClient};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

fn client_for(port: u16, workspace: &Path) -> OpenCodeClient {
    OpenCodeClient::new(AgentEndpoint {
        base_url: format!("http://127.0.0.1:{port}"),
        provider_id: "provider".into(),
        model_id: "selected-model-b".into(),
        workspace: workspace.to_path_buf(),
        auth_header: String::new(),
    })
}

fn read_request(stream: &mut std::net::TcpStream) -> Result<String, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    loop {
        let mut buffer = [0_u8; 4096];
        let count = stream
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        if bytes.len() >= header_end + 4 + length {
            break;
        }
    }
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

#[test]
fn workspace_directory_scopes_every_session_request() -> Result<(), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let server = std::thread::spawn(move || {
        for _ in 0..4 {
            let (mut stream, _) = listener.accept().map_err(|error| error.to_string())?;
            let request = read_request(&mut stream)?;
            let target = request
                .split_whitespace()
                .nth(1)
                .unwrap_or_default()
                .to_owned();
            let path = url::Url::parse(&format!("http://127.0.0.1{target}"))
                .map_err(|error| error.to_string())?
                .path()
                .to_owned();
            captured
                .lock()
                .map_err(|_| "requests lock".to_owned())?
                .push(request);
            let body = match path.as_str() {
                "/session" => "{\"id\":\"session-unicode\"}",
                "/session/session-unicode/message" => {
                    r#"[{"info":{"id":"msg-native","sessionID":"session-unicode","role":"assistant","parentID":"message","system":"<!-- yume:persona=baobao -->","providerID":"provider","modelID":"selected-model-b","tokens":{"input":2,"output":3,"reasoning":4,"cache":{"read":5,"write":6}},"time":{"created":7,"completed":8}},"parts":[{"id":"prt-native","type":"text","text":"native reply"}]}]"#
                }
                "/session/session-unicode/abort" => "true",
                _ => "{}",
            };
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len()).map_err(|error| error.to_string())?;
            stream.flush().map_err(|error| error.to_string())?;
            stream
                .shutdown(std::net::Shutdown::Write)
                .map_err(|error| error.to_string())?;
        }
        Ok::<(), String>(())
    });
    let workspace = Path::new("\\\\?\\E:\\工作 伙伴\\保留#?&%");
    let client = client_for(port, workspace);
    assert_eq!(client.create_session(workspace)?, "session-unicode");
    client.prompt("session-unicode", "message", "system", "input")?;
    let snapshot = client.snapshot("session-unicode")?;
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].role.as_deref(), Some("assistant"));
    assert_eq!(snapshot[0].created, Some(7));
    assert_eq!(snapshot[0].system.as_deref(), Some("<!-- yume:persona=baobao -->"));
    assert_eq!(snapshot[0].parent_id.as_deref(), Some("message"));
    let usage = snapshot[0].usage.as_ref().expect("completed snapshot retains native usage");
    assert_eq!(usage.message_id, "msg-native");
    assert_eq!(usage.sidecar_id, "provider");
    assert_eq!(usage.model_id, "selected-model-b");
    assert_eq!(usage.created_at_ms, 7);
    assert_eq!(usage.tokens, 20);
    assert_eq!(snapshot[0].parts[0].kind.as_deref(), Some("text"));
    assert_eq!(snapshot[0].parts[0].text.as_deref(), Some("native reply"));
    assert!(client.abort("session-unicode")?);
    server.join().map_err(|_| "server join".to_owned())??;
    let requests = requests.lock().map_err(|_| "requests lock".to_owned())?;
    assert_eq!(requests.len(), 4);
    let prompt_body = requests[1]
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .ok_or("missing prompt body")?;
    let prompt: serde_json::Value =
        serde_json::from_str(prompt_body).map_err(|error| error.to_string())?;
    assert_eq!(
        prompt["model"],
        serde_json::json!({"providerID":"provider","modelID":"selected-model-b"})
    );
    let create_body = requests[0]
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .ok_or_else(|| "missing create session body".to_owned())?;
    let create: serde_json::Value =
        serde_json::from_str(create_body).map_err(|error| error.to_string())?;
    assert_eq!(
        create.get("permission"),
        Some(&serde_json::json!([
            {"permission":"read","pattern":"*","action":"ask"},
            {"permission":"glob","pattern":"*","action":"ask"},
            {"permission":"grep","pattern":"*","action":"ask"},
            {"permission":"list","pattern":"*","action":"ask"},
            {"permission":"write","pattern":"*","action":"ask"},
            {"permission":"edit","pattern":"*","action":"ask"},
            {"permission":"patch","pattern":"*","action":"ask"},
            {"permission":"bash","pattern":"*","action":"ask"},
            {"permission":"webfetch","pattern":"*","action":"deny"},
            {"permission":"websearch","pattern":"*","action":"deny"}
        ]))
    );
    for request in requests.iter() {
        let target = request.split_whitespace().nth(1).unwrap_or_default();
        let url = url::Url::parse(&format!("http://127.0.0.1{target}"))
            .map_err(|error| error.to_string())?;
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "directory")
                .map(|(_, value)| value.into_owned()),
            Some("E:\\工作 伙伴\\保留#?&%".to_owned())
        );
        assert!(!request
            .to_ascii_lowercase()
            .contains("x-opencode-directory:"));
    }
    Ok(())
}

#[test]
fn recovery_snapshot_is_one_get_and_never_posts() -> Result<(), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().map_err(|error| error.to_string())?;
        let request = read_request(&mut stream)?;
        if !request.starts_with("GET /session/ses_old/message?") {
            return Err(format!("unexpected recovery request: {request}"));
        }
        let body = "[]";
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len()).map_err(|error| error.to_string())?;
        stream.flush().map_err(|error| error.to_string())?;
        Ok::<(), String>(())
    });
    let workspace = Path::new("E:\\synthetic-old-workspace");
    assert!(client_for(port, workspace).snapshot("ses_old")?.is_empty());
    server.join().map_err(|_| "server join".to_owned())??;
    Ok(())
}

#[test]
fn accepted_prompt_confirmation_recovers_from_native_facts_without_replay() -> Result<(), String> {
    use super::{AgentPermissionState, AgentRunState, RunStore};
    let root = std::env::temp_dir().join(format!("yume-confirm-http-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let store = RunStore::new(root.join("runs"));
    let state = AgentRunState::new(store.clone());
    state.begin("msg_confirm", &root, "once")?;
    state.bind_session("msg_confirm", "ses_confirm")?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let server = std::thread::spawn(move || -> Result<usize, String> {
        let mut prompts = 0;
        for index in 0..3 {
            let (mut stream, _) = listener.accept().map_err(|e| e.to_string())?;
            let request = read_request(&mut stream)?;
            let body = if index == 0 {
                assert!(request.starts_with("POST /session/ses_confirm/prompt_async?"));
                prompts += 1;
                "{}"
            } else {
                assert!(request.starts_with("GET /session/ses_confirm/message?"));
                r#"[{"info":{"id":"msg_confirm","role":"user","time":{"created":1}},"parts":[]}]"#
            };
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len()).map_err(|e| e.to_string())?;
        }
        Ok(prompts)
    });
    let client = client_for(port, &root);
    client.prompt("ses_confirm", "msg_confirm", "", "once")?;
    store.inject_write_failure(true);
    assert!(state.confirm_submission("msg_confirm").is_err());
    let restarted = AgentRunState::load(store.clone())?;
    let candidate = restarted.active_record("msg_confirm")?;
    assert!(
        super::recovery::confirm_from_native_with(&restarted, &candidate, || client
            .snapshot("ses_confirm"))
        .is_err()
    );
    assert!(restarted
        .active_record("msg_confirm")?
        .initial_input
        .is_some());
    store.inject_write_failure(false);
    assert!(super::recovery::confirm_from_native_with(
        &restarted,
        &candidate,
        || client.snapshot("ses_confirm")
    )?);
    assert!(restarted
        .active_record("msg_confirm")?
        .initial_input
        .is_none());
    let permissions = AgentPermissionState::default();
    permissions.ensure_run_registered(&restarted.active_record("msg_confirm")?, &[])?;
    assert!(permissions.waiting("msg_confirm").is_ok());
    assert_eq!(server.join().map_err(|_| "join")??, 1);
    std::fs::remove_dir_all(root).map_err(|e| e.to_string())?;
    Ok(())
}

#[test]
fn incomplete_usage_never_disrupts_snapshot_and_invalid_counts_are_not_captured() -> Result<(), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let port = listener.local_addr().map_err(|error| error.to_string())?.port();
    let base = serde_json::json!({"info": {
        "id":"msg-usage", "sessionID":"session-usage", "role":"assistant", "parentID":"message", "providerID":"provider", "modelID":"model",
        "time":{"created":7,"completed":8}, "tokens":{"input":2,"output":3,"reasoning":4,"cache":{"read":5,"write":6}}
    }, "parts":[]});
    let cases: Vec<(&str, serde_json::Value, Option<u64>)> = vec![
        ("sessionID", serde_json::Value::Null, None),
        ("providerID", serde_json::Value::Null, None),
        ("modelID", serde_json::json!(""), None),
        ("providerID", serde_json::json!(17), None),
        ("modelID", serde_json::json!({"invalid":true}), None),
        ("tokens", serde_json::Value::Null, None),
        ("tokens", serde_json::json!("invalid"), None),
        ("tokens", serde_json::json!({}), None),
        ("tokens", serde_json::json!({"input":100_000_001,"output":0,"reasoning":0,"cache":{"read":0,"write":0}}), None),
        ("tokens", serde_json::json!({"input":u64::MAX,"output":1,"reasoning":0,"cache":{"read":0,"write":0}}), None),
        ("tokens", serde_json::json!({"input":0,"output":0,"reasoning":0,"cache":{"read":0,"write":0}}), Some(0)),
        ("tokens", serde_json::json!({"input":0,"output":0}), None),
        ("tokens", serde_json::json!({"input":-1,"output":"invalid","reasoning":4,"cache":{"read":0,"write":0}}), None),
        ("tokens", serde_json::json!({"input":0,"output":0,"reasoning":0,"cache":{"read":"invalid","write":0}}), None),
        ("role", serde_json::json!("user"), None),
        ("time", serde_json::json!({"created":7}), None),
        ("time", serde_json::json!({"completed":8}), None),
        ("time", serde_json::json!({"created":u64::MAX,"completed":8}), None),
    ];
    let expected: Vec<Option<u64>> = cases.iter().map(|(_, _, expected)| *expected).collect();
    let payloads: Vec<String> = cases.into_iter().map(|(field, value, _)| {
        let mut message = base.clone();
        if value.is_null() { message["info"].as_object_mut().unwrap().remove(field); }
        else { message["info"][field] = value; }
        serde_json::json!([message]).to_string()
    }).collect();
    let server = std::thread::spawn(move || -> Result<(), String> {
        for body in payloads {
            let (mut stream, _) = listener.accept().map_err(|error| error.to_string())?;
            assert!(read_request(&mut stream)?.starts_with("GET /session/session-usage/message?"));
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len()).map_err(|error| error.to_string())?;
            stream.flush().map_err(|error| error.to_string())?;
        }
        Ok(())
    });
    let client = client_for(port, Path::new(r"E:\synthetic-usage-workspace"));
    for expected in expected {
        let messages = client.snapshot("session-usage")?;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].usage.as_ref().map(|usage| usage.tokens), expected);
    }
    server.join().map_err(|_| "usage server join".to_owned())??;
    Ok(())
}

#[test]
fn snapshot_rejects_a_different_or_invalid_session_before_capture() -> Result<(), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let port = listener.local_addr().map_err(|error| error.to_string())?.port();
    let server = std::thread::spawn(move || -> Result<(), String> {
        for session in [serde_json::json!("another-session"), serde_json::json!(17)] {
            let (mut stream, _) = listener.accept().map_err(|error| error.to_string())?;
            assert!(read_request(&mut stream)?.starts_with("GET /session/session-usage/message?"));
            let body = serde_json::json!([{"info":{
                "id":"msg-usage", "sessionID":session, "role":"assistant", "parentID":"message",
                "providerID":"provider", "modelID":"model", "time":{"created":7,"completed":8},
                "tokens":{"input":2,"output":3,"reasoning":4,"cache":{"read":5,"write":6}}
            },"parts":[]}]).to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len()).map_err(|error| error.to_string())?;
            stream.flush().map_err(|error| error.to_string())?;
        }
        Ok(())
    });
    let client = client_for(port, Path::new(r"E:\synthetic-usage-workspace"));
    for _ in 0..2 {
        assert_eq!(client.snapshot("session-usage").unwrap_err(), "agent_snapshot_session_mismatch");
    }
    server.join().map_err(|_| "usage session server join".to_owned())??;
    Ok(())
}

fn single_message_response(
    status: &'static str,
    body: serde_json::Value,
) -> Result<(u16, std::thread::JoinHandle<Result<String, String>>), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    let port = listener.local_addr().map_err(|error| error.to_string())?.port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().map_err(|error| error.to_string())?;
        let request = read_request(&mut stream)?;
        let body = body.to_string();
        write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len()).map_err(|error| error.to_string())?;
        stream.flush().map_err(|error| error.to_string())?;
        Ok(request)
    });
    Ok((port, server))
}

fn authoritative_message() -> serde_json::Value {
    serde_json::json!({"info": {
        "id":"msg_native", "sessionID":"ses_native", "role":"assistant", "parentID":"msg_user",
        "system":"system", "finish":"stop", "error":{"name":"ProviderError"},
        "providerID":"provider", "modelID":"model", "time":{"created":7,"completed":8},
        "tokens":{"input":2,"output":3,"reasoning":4,"cache":{"read":5,"write":6}}
    }, "parts":[
        {"id":"prt_text","messageID":"msg_native","sessionID":"ses_native","type":"text","text":"reply","synthetic":true},
        {"id":"prt_tool","messageID":"msg_native","sessionID":"ses_native","type":"tool",
         "callID":"call_native","tool":"read","state":{"status":"completed","input":{"path":"a"},
         "output":{"text":"contents"},"metadata":{"title":"a"}}},
        {"id":"prt_error","messageID":"msg_native","sessionID":"ses_native","type":"tool","callID":"call_error","tool":"bash",
         "state":{"status":"error","input":{},"output":"stale","error":"denied","metadata":{}}}
    ]})
}

#[test]
fn single_message_get_preserves_complete_native_fields_directory_and_auth() -> Result<(), String> {
    let (port, server) = single_message_response("200 OK", authoritative_message())?;
    let client = OpenCodeClient::new(AgentEndpoint {
        base_url: format!("http://127.0.0.1:{port}"),
        provider_id: "provider".into(), model_id: "model".into(),
        workspace: Path::new(r"\\?\E:\工作 伙伴\保留#?&%").to_path_buf(),
        auth_header: "Basic Zml4dHVyZTpwYXNzd29yZA==".into(),
    });
    let message = client.message("ses_native", "msg_native")?;
    assert_eq!(message.id, "msg_native");
    assert_eq!(message.system.as_deref(), Some("system"));
    assert_eq!(message.role.as_deref(), Some("assistant"));
    assert_eq!(message.parent_id.as_deref(), Some("msg_user"));
    assert_eq!(message.created, Some(7));
    assert!(message.completed);
    assert_eq!(message.finish.as_deref(), Some("stop"));
    assert_eq!(message.error.as_deref(), Some("ProviderError"));
    let usage = message.usage.as_ref().expect("complete authoritative usage");
    assert_eq!((&*usage.message_id, &*usage.sidecar_id, &*usage.model_id), ("msg_native", "provider", "model"));
    assert_eq!((usage.created_at_ms, usage.tokens), (7, 20));
    assert_eq!(message.parts.len(), 3);
    assert_eq!(message.parts[0].id, "prt_text");
    assert!(message.parts[0].synthetic);
    assert_eq!(message.parts[0].text.as_deref(), Some("reply"));
    let tool = &message.parts[1];
    assert_eq!(tool.kind.as_deref(), Some("tool"));
    assert_eq!(tool.call_id.as_deref(), Some("call_native"));
    assert_eq!(tool.tool.as_deref(), Some("read"));
    assert!(!tool.synthetic);
    let state = tool.state.as_ref().expect("tool state");
    assert_eq!(state.status, "completed");
    assert_eq!(state.input, serde_json::json!({"path":"a"}));
    assert_eq!(state.output, serde_json::json!({"text":"contents"}));
    assert_eq!(state.metadata, serde_json::json!({"title":"a"}));
    assert_eq!(message.parts[2].state.as_ref().unwrap().output, serde_json::json!("denied"));
    let request = server.join().map_err(|_| "message server join")??;
    assert!(request.starts_with("GET /session/ses_native/message/msg_native?"));
    assert!(request.lines().any(|line| line == "Authorization: Basic Zml4dHVyZTpwYXNzd29yZA=="));
    let target = request.split_whitespace().nth(1).ok_or("missing request target")?;
    let url = url::Url::parse(&format!("http://127.0.0.1{target}")).map_err(|error| error.to_string())?;
    assert_eq!(url.query_pairs().collect::<Vec<_>>(), vec![("directory".into(), "E:\\工作 伙伴\\保留#?&%".into())]);
    assert!(url.fragment().is_none());
    Ok(())
}

#[test]
fn single_message_rejects_missing_or_mismatched_message_and_part_identities() -> Result<(), String> {
    let mut cases = Vec::new();
    let mut wrong_id = authoritative_message();
    wrong_id["info"]["id"] = serde_json::json!("msg_other");
    cases.push(wrong_id);
    let mut missing_session = authoritative_message();
    missing_session["info"].as_object_mut().unwrap().remove("sessionID");
    cases.push(missing_session);
    for value in [serde_json::json!("ses_other"), serde_json::json!(17), serde_json::Value::Null] {
        let mut message = authoritative_message();
        message["info"]["sessionID"] = value;
        cases.push(message);
    }
    for field in ["messageID", "sessionID"] {
        for value in [serde_json::json!("other"), serde_json::json!(17), serde_json::Value::Null] {
            let mut message = authoritative_message();
            message["parts"][0][field] = value;
            cases.push(message);
        }
    }
    for body in cases {
        let (port, server) = single_message_response("200 OK", body)?;
        assert_eq!(client_for(port, Path::new("E:\\fixture")).message("ses_native", "msg_native").unwrap_err(), "agent_message_identity_mismatch");
        assert!(server.join().map_err(|_| "identity server join")??.starts_with("GET /session/ses_native/message/msg_native?"));
    }
    Ok(())
}

#[test]
fn single_message_rejects_unsafe_path_ids_before_http() -> Result<(), String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|error| error.to_string())?;
    listener.set_nonblocking(true).map_err(|error| error.to_string())?;
    let client = client_for(listener.local_addr().map_err(|error| error.to_string())?.port(), Path::new("E:\\fixture"));
    for invalid in ["", ".", "..", "../other", "a/b", "a\\b", "a?directory=x", "a#x", "%2f", "a b", "a\r\nb", "消息", &"a".repeat(129)] {
        assert_eq!(client.message(invalid, "msg_native").unwrap_err(), "agent_invalid_id");
        assert_eq!(client.message("ses_native", invalid).unwrap_err(), "agent_invalid_id");
    }
    assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
    Ok(())
}

#[test]
fn single_message_keeps_404_and_malformed_payload_errors() -> Result<(), String> {
    for (status, body, expected) in [
        ("404 Not Found", serde_json::json!({"error":"not found"}), "agent_http_404"),
        ("200 OK", serde_json::json!({"info":{},"parts":[]}), "agent_wire_invalid"),
    ] {
        let (port, server) = single_message_response(status, body)?;
        assert_eq!(client_for(port, Path::new("E:\\fixture")).message("ses_native", "msg_native").unwrap_err(), expected);
        assert!(server.join().map_err(|_| "error server join")??.starts_with("GET /session/ses_native/message/msg_native?"));
    }
    Ok(())
}

#[test]
fn full_snapshot_keeps_legacy_optional_identity_behavior() -> Result<(), String> {
    let mut message = authoritative_message();
    message["info"].as_object_mut().unwrap().remove("sessionID");
    message["parts"][0]["sessionID"] = serde_json::Value::Null;
    message["parts"][0]["messageID"] = serde_json::json!(17);
    let (port, server) = single_message_response("200 OK", serde_json::json!([message]))?;
    let snapshot = client_for(port, Path::new("E:\\fixture")).snapshot("ses_native")?;
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].id, "msg_native");
    assert!(snapshot[0].usage.is_none());
    assert_eq!(snapshot[0].parts.len(), 3);
    assert_eq!(snapshot[0].parts[0].text.as_deref(), Some("reply"));
    assert!(server.join().map_err(|_| "snapshot server join")??.starts_with("GET /session/ses_native/message?"));
    Ok(())
}

#[test]
fn single_message_rejects_incomplete_structures_before_cache_replacement() -> Result<(), String> {
    let mut cases = Vec::new();
    for field in ["role", "time", "parentID"] {
        let mut message = authoritative_message();
        if field == "time" { message["info"]["time"] = serde_json::json!({}); }
        else { message["info"].as_object_mut().unwrap().remove(field); }
        cases.push(message);
    }
    for role in ["", "system", "tool"] {
        let mut message = authoritative_message();
        message["info"]["role"] = serde_json::json!(role);
        cases.push(message);
    }
    for field in ["type", "text", "messageID", "sessionID"] {
        let mut message = authoritative_message();
        message["parts"][0].as_object_mut().unwrap().remove(field);
        cases.push(message);
    }
    for field in ["state", "callID", "tool"] {
        let mut message = authoritative_message();
        message["parts"][1].as_object_mut().unwrap().remove(field);
        cases.push(message);
    }
    for field in ["id", "type"] {
        let mut message = authoritative_message();
        message["parts"][0][field] = serde_json::json!("");
        cases.push(message);
    }
    for field in ["callID", "tool"] {
        let mut message = authoritative_message();
        message["parts"][1][field] = serde_json::json!("");
        cases.push(message);
    }
    for status in ["", "unknown"] {
        let mut message = authoritative_message();
        message["parts"][1]["state"]["status"] = serde_json::json!(status);
        cases.push(message);
    }
    for parent in ["", "../other"] {
        let mut message = authoritative_message();
        message["info"]["parentID"] = serde_json::json!(parent);
        cases.push(message);
    }
    let mut duplicate = authoritative_message();
    duplicate["parts"][1]["id"] = duplicate["parts"][0]["id"].clone();
    cases.push(duplicate);
    let mut reasoning = authoritative_message();
    reasoning["parts"][0]["type"] = serde_json::json!("reasoning");
    reasoning["parts"][0].as_object_mut().unwrap().remove("text");
    cases.push(reasoning);
    cases.push(serde_json::json!({"info":{"id":"msg_native","sessionID":"ses_native","time":{}},"parts":[]}));
    for body in cases {
        let (port, server) = single_message_response("200 OK", body)?;
        assert_eq!(client_for(port, Path::new("E:\\fixture")).message("ses_native", "msg_native").unwrap_err(), "agent_message_incomplete");
        server.join().map_err(|_| "incomplete server join")??;
    }
    Ok(())
}

#[test]
fn single_message_accepts_valid_user_assistant_empty_parts_and_empty_text() -> Result<(), String> {
    for role in ["user", "assistant"] {
        for parts in [serde_json::json!([]), serde_json::json!([
            {"id":"prt_text","messageID":"msg_native","sessionID":"ses_native","type":"text","text":""},
            {"id":"prt_reasoning","messageID":"msg_native","sessionID":"ses_native","type":"reasoning","text":""}
        ])] {
            let mut body = serde_json::json!({"info":{"id":"msg_native","sessionID":"ses_native","role":role,"time":{"created":0}},"parts":parts});
            if role == "assistant" { body["info"]["parentID"] = serde_json::json!("msg_user"); }
            let (port, server) = single_message_response("200 OK", body)?;
            let message = client_for(port, Path::new("E:\\fixture")).message("ses_native", "msg_native")?;
            assert_eq!(message.role.as_deref(), Some(role));
            assert_eq!(message.created, Some(0));
            assert!(!message.completed);
            assert!(message.usage.is_none());
            server.join().map_err(|_| "valid message server join")??;
        }
    }
    Ok(())
}


#[test]
fn snapshot_preserves_native_file_names_without_resolving_urls() -> Result<(), String> {
    let body = serde_json::json!([{
        "info": {"id":"msg_file","sessionID":"ses_native","role":"user","time":{"created":1}},
        "parts": [
            {"id":"p_pdf","type":"file","filename":"b7.pdf","mime":"text/plain","url":"data:text/plain;base64,cHJpdmF0ZQ=="},
            {"id":"p_remote","type":"file","filename":"remote.txt","mime":"text/plain","url":"https://invalid.example/never-fetch"},
            {"id":"p_text","type":"text","filename":"spoof.txt","text":"question"}
        ]
    }]);
    let (port, server) = single_message_response("200 OK", body)?;
    let messages = client_for(port, Path::new(".")).snapshot("ses_native")?;
    assert_eq!(messages[0].parts[0].file_name.as_deref(), Some("b7.pdf"));
    assert_eq!(messages[0].parts[1].file_name.as_deref(), Some("remote.txt"));
    assert!(messages[0].parts[2].file_name.is_none());
    let debug = format!("{:?}", messages[0]);
    assert!(!debug.contains("base64"));
    assert!(!debug.contains("invalid.example"));
    assert!(server.join().map_err(|_| "file server join")??.starts_with("GET /session/ses_native/message?"));
    Ok(())
}
