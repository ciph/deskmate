use super::*;
use crate::chat_attachments::{
    AttachmentStore, DiscardChatAttachmentRequest, ReadChatAttachmentRequest,
};
use serde_json::{json, Value};
use tauri::http::HeaderValue;

const MIB: usize = 1024 * 1024;

fn metadata(size: usize) -> Value {
    json!({"uploadId":"upload-1:2", "scopeKey":"generation-3", "sessionId":"session-a",
        "fileName":"notes.txt", "mime":"text/plain", "size":size})
}

fn headers(value: &Value) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        HEADER,
        HeaderValue::from_str(
            &base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(value).unwrap()),
        )
        .unwrap(),
    );
    headers
}

#[test]
fn binary_roundtrip_preserves_raw_bytes_and_scope_receipt() {
    let root = tempfile::tempdir().unwrap();
    let store = AttachmentStore::default();
    for size in [MIB, 8 * MIB, 20 * MIB] {
        let body = InvokeBody::Raw(vec![b'x'; size]);
        let (metadata, bytes) =
            parse(&body, &headers(&metadata(size)), UploadKind::Attachment).unwrap();
        assert_eq!(
            bytes.as_ptr(),
            match &body {
                InvokeBody::Raw(bytes) => bytes.as_ptr(),
                _ => unreachable!(),
            }
        );
        let staged = store
            .stage(root.path(), metadata.stage_request(bytes.to_vec()))
            .unwrap();
        let wire = serde_json::to_value(metadata.receipt(&staged)).unwrap();
        assert_eq!(wire["uploadId"], "upload-1:2");
        assert_eq!(wire["scopeKey"], "generation-3");
        assert_eq!(wire["value"]["size"], size);
        assert!(wire["value"].get("path").is_none());
        let read = store
            .read(ReadChatAttachmentRequest {
                session_id: "session-a".into(),
                attachment_id: staged.id.clone(),
            })
            .unwrap();
        let encoded = read
            .data_url
            .strip_prefix("data:text/plain;base64,")
            .unwrap();
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .unwrap(),
            bytes
        );
        store
            .discard(DiscardChatAttachmentRequest {
                session_id: "session-a".into(),
                attachment_id: staged.id,
            })
            .unwrap();
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn raw_body_is_mandatory_and_metadata_is_bounded_and_strict() {
    let raw = InvokeBody::Raw(vec![1]);
    assert!(parse(
        &InvokeBody::Json(json!([1])),
        &headers(&metadata(1)),
        UploadKind::Attachment
    )
    .is_err());
    assert!(parse(&raw, &HeaderMap::new(), UploadKind::Attachment).is_err());
    for encoded in [
        "!invalid!".to_owned(),
        "A".repeat(MAX_HEADER_BYTES + 1),
        base64::engine::general_purpose::STANDARD.encode([0xff]),
        base64::engine::general_purpose::STANDARD.encode(b"{bad-json"),
    ] {
        let mut headers = HeaderMap::new();
        headers.insert(HEADER, HeaderValue::from_str(&encoded).unwrap());
        assert!(parse(&raw, &headers, UploadKind::Attachment).is_err());
    }
    let mut duplicate = headers(&metadata(1));
    duplicate.append(HEADER, duplicate[HEADER].clone());
    assert!(parse(&raw, &duplicate, UploadKind::Attachment).is_err());
    for (key, value) in [
        ("unexpected", json!(true)),
        ("size", json!(2)),
        ("size", json!(-1)),
        ("size", json!(1.5)),
        ("sessionId", Value::Null),
        ("sessionId", json!("../bad")),
        ("uploadId", json!("bad token")),
        ("uploadId", json!("x".repeat(129))),
        ("scopeKey", json!(" ")),
        ("scopeKey", json!("bad\nkey")),
        ("scopeKey", json!("x".repeat(1025))),
        ("fileName", json!("CON.txt")),
        ("fileName", json!("name.txt.")),
        ("fileName", json!("x".repeat(256))),
        ("mime", json!("image/fake")),
    ] {
        let mut value_json = metadata(1);
        value_json[key] = value;
        assert!(
            parse(&raw, &headers(&value_json), UploadKind::Attachment).is_err(),
            "{value_json}"
        );
    }
    assert!(parse(
        &InvokeBody::Raw(vec![]),
        &headers(&metadata(0)),
        UploadKind::Attachment
    )
    .is_err());
}

#[test]
fn binary_ordinary_and_ncm_limits_and_store_quota_are_preserved() {
    let body = InvokeBody::Raw(vec![1; 20 * MIB + 1]);
    assert!(parse(
        &body,
        &headers(&metadata(20 * MIB + 1)),
        UploadKind::Attachment
    )
    .unwrap_err()
    .contains("20 MiB"));
    let root = tempfile::tempdir().unwrap();
    let store = AttachmentStore::default();
    let body = InvokeBody::Raw(vec![1; 20 * MIB]);
    let (meta, bytes) =
        parse(&body, &headers(&metadata(20 * MIB)), UploadKind::Attachment).unwrap();
    let staged = store
        .stage(root.path(), meta.stage_request(bytes.to_vec()))
        .unwrap();
    let (extra, bytes) = parse(
        &InvokeBody::Raw(vec![1]),
        &headers(&metadata(1)),
        UploadKind::Attachment,
    )
    .map(|(metadata, bytes)| (metadata, bytes.to_vec()))
    .unwrap();
    assert!(store
        .stage(root.path(), extra.stage_request(bytes.clone()))
        .is_err());
    store
        .discard(DiscardChatAttachmentRequest {
            session_id: "session-a".into(),
            attachment_id: staged.id,
        })
        .unwrap();
    store
        .stage(root.path(), extra.stage_request(bytes))
        .unwrap();
    for size in [64 * MIB, 64 * MIB + 1] {
        let body = InvokeBody::Raw(vec![1; size]);
        let mut meta = metadata(size);
        meta["mime"] = json!("application/x-ncm");
        meta["fileName"] = json!("song.ncm");
        let result = parse(&body, &headers(&meta), UploadKind::Attachment);
        assert_eq!(result.is_ok(), size == 64 * MIB);
    }
}

#[test]
fn basic_image_signatures_cannot_be_relabelled() {
    for (mime, name, bytes) in [
        ("image/png", "image.png", b"\x89PNG\r\n\x1a\n".as_slice()),
        ("image/jpeg", "image.jpg", &[0xff, 0xd8, 0xff]),
        ("image/gif", "image.gif", b"GIF89a".as_slice()),
        ("image/webp", "image.webp", b"RIFF0000WEBP".as_slice()),
    ] {
        let mut meta = metadata(bytes.len());
        meta["mime"] = json!(mime);
        meta["fileName"] = json!(name);
        assert!(parse(
            &InvokeBody::Raw(bytes.to_vec()),
            &headers(&meta),
            UploadKind::Attachment
        )
        .is_ok());
        assert!(parse(
            &InvokeBody::Raw(vec![b'x'; bytes.len()]),
            &headers(&meta),
            UploadKind::Attachment
        )
        .is_err());
    }
}

#[test]
fn media_header_preserves_body_limits_and_rejects_false_mime() {
    for size in [1, MIB, 8 * MIB, 64 * MIB, 64 * MIB + 1] {
        let mut meta = metadata(size);
        meta.as_object_mut().unwrap().remove("sessionId");
        meta["fileName"] = json!("音频.mp3");
        meta["mime"] = json!("audio/mpeg");
        let body = InvokeBody::Raw(vec![1; size]);
        assert_eq!(
            parse(&body, &headers(&meta), UploadKind::Media).is_ok(),
            size <= 64 * MIB
        );
        for bad_mime in ["audio/evil", "video/mp4", "text/plain"] {
            meta["mime"] = json!(bad_mime);
            assert!(parse(&body, &headers(&meta), UploadKind::Media).is_err());
        }
    }
}

#[test]
fn tauri_mock_command_routes_enforce_chat_and_raw_metadata_before_staging() {
    use crate::chat_attachments::resources::ResourceStore;
    use tauri::Manager;
    let probe = tauri::test::mock_app();
    let isolated = tempfile::Builder::new()
        .prefix("yume-binary-ipc-test-")
        .tempdir_in(probe.path().cache_dir().unwrap())
        .unwrap();
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().identifier = isolated
        .path()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let app = tauri::test::mock_builder()
        .manage(AttachmentStore::default())
        .manage(ResourceStore::open(&isolated.path().join("resources.sqlite3")).unwrap())
        .invoke_handler(tauri::generate_handler![
            crate::chat_attachments::stage_chat_attachment_binary,
            crate::chat_attachments::resources::stage_chat_resource_upload_binary,
        ])
        .build(context)
        .unwrap();
    assert_eq!(app.path().app_cache_dir().unwrap(), isolated.path());
    let chat = tauri::WebviewWindowBuilder::new(&app, "chat", Default::default())
        .build()
        .unwrap();
    let pet = tauri::WebviewWindowBuilder::new(&app, "pet", Default::default())
        .build()
        .unwrap();
    let invoke =
        |window: &tauri::WebviewWindow<tauri::test::MockRuntime>, command: &str, body, headers| {
            tauri::test::get_ipc_response(
                window,
                tauri::webview::InvokeRequest {
                    cmd: command.to_owned(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    url: if cfg!(windows) {
                        "http://tauri.localhost"
                    } else {
                        "tauri://localhost"
                    }
                    .parse()
                    .unwrap(),
                    body,
                    headers,
                    invoke_key: tauri::test::INVOKE_KEY.to_owned(),
                },
            )
            .map(|response| response.deserialize::<Value>().unwrap())
        };
    for command in [
        "stage_chat_attachment_binary",
        "stage_chat_resource_upload_binary",
    ] {
        let mut meta = metadata(4);
        if command.contains("resource") {
            meta.as_object_mut().unwrap().remove("sessionId");
            meta["fileName"] = json!("recording.webm");
            meta["mime"] = json!("audio/webm;codecs=opus");
        }
        assert_eq!(
            invoke(
                &pet,
                command,
                InvokeBody::Raw(b"note".to_vec()),
                headers(&meta)
            ),
            Err(json!("permission_chat_only"))
        );
        assert_eq!(
            invoke(
                &chat,
                command,
                InvokeBody::Json(json!({"bytes":[1,2,3,4]})),
                headers(&meta)
            ),
            Err(json!("upload_binary_body_required"))
        );
        assert_eq!(
            invoke(
                &chat,
                command,
                InvokeBody::Raw(b"note".to_vec()),
                HeaderMap::new()
            ),
            Err(json!("upload_metadata_required"))
        );
        let response = invoke(
            &chat,
            command,
            InvokeBody::Raw(b"note".to_vec()),
            headers(&meta),
        )
        .unwrap();
        assert_eq!(response["uploadId"], meta["uploadId"]);
        assert_eq!(response["scopeKey"], meta["scopeKey"]);
        assert_eq!(response["value"]["size"], 4);
        assert!(response["value"].get("path").is_none());
        let id = response["value"]["id"].as_str().unwrap().to_owned();
        if !command.contains("resource") {
            app.state::<AttachmentStore>()
                .discard(DiscardChatAttachmentRequest {
                    session_id: "session-a".into(),
                    attachment_id: id,
                })
                .unwrap();
        }
    }
    for command in ["stage_chat_attachment", "stage_chat_resource_upload"] {
        assert!(invoke(
            &chat,
            command,
            InvokeBody::Json(json!({})),
            HeaderMap::new()
        )
        .is_err());
    }
}
