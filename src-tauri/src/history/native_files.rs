//! Transient display metadata from scoped native snapshots. Never persisted in history.json.
use super::HistoryMessage;
use crate::agent::NativeMessage;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize)]
pub(crate) struct FileLabel {
    id: String,
    name: String,
}

pub(super) struct MessageFiles {
    message_id: String,
    time: u64,
    files: Vec<FileLabel>,
}

#[derive(Serialize)]
pub(crate) struct CatalogMessage {
    #[serde(flatten)]
    message: HistoryMessage,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attachments: Vec<FileLabel>,
}
impl From<HistoryMessage> for CatalogMessage {
    fn from(message: HistoryMessage) -> Self {
        Self { message, attachments: Vec::new() }
    }
}

pub(super) fn collect(snapshot: &[NativeMessage]) -> Vec<MessageFiles> {
    snapshot.iter().filter(|message| message.role.as_deref() == Some("user"))
        .filter_map(|message| {
            let mut seen = HashSet::new();
            let files: Vec<_> = message.parts.iter().filter_map(|part| {
                if part.synthetic || part.kind.as_deref() != Some("file") || part.id.is_empty() {
                    return None;
                }
                let name = part.file_name.as_deref()?.rsplit(['/', '\\']).next()?.trim();
                if name.is_empty() || name.chars().count() > 1024 || !seen.insert(&part.id) {
                    return None;
                }
                Some(FileLabel { id: part.id.clone(), name: name.to_owned() })
            }).collect();
            (!files.is_empty()).then(|| MessageFiles {
                message_id: message.id.clone(), time: message.created.unwrap_or(0), files,
            })
        }).collect()
}

pub(super) fn include_file_only_messages(messages: &mut Vec<HistoryMessage>, files: &[MessageFiles]) {
    if files.is_empty() { return; }
    let existing: HashSet<_> = messages.iter().filter_map(|message| message.message_id.clone()).collect();
    for item in files {
        if existing.contains(&item.message_id) { continue; }
        messages.push(HistoryMessage {
            role: "user".into(), text: String::new(), time: item.time,
            message_id: Some(item.message_id.clone()), part_id: None,
            persona_id: None, local_only: false,
        });
    }
    // Stable ordering keeps native part order when timestamps coincide.
    messages.sort_by_key(|message| message.time);
}

pub(super) fn attach(messages: Vec<HistoryMessage>, files: Vec<MessageFiles>) -> Vec<CatalogMessage> {
    let mut files: HashMap<_, _> = files.into_iter().map(|item| (item.message_id, item.files)).collect();
    messages.into_iter().map(|message| {
        let attachments = if message.role == "user" && !message.local_only {
            message.message_id.as_ref().and_then(|id| files.remove(id)).unwrap_or_default()
        } else { Vec::new() };
        CatalogMessage { message, attachments }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::NativePart;
    fn part(id: &str, kind: &str, name: Option<&str>, synthetic: bool) -> NativePart {
        NativePart { id: id.into(), kind: Some(kind.into()), file_name: name.map(str::to_owned),
            synthetic, text: None, call_id: None, tool: None, state: None }
    }
    fn native(id: &str, role: &str, time: u64, parts: Vec<NativePart>) -> NativeMessage {
        NativeMessage { id: id.into(), role: Some(role.into()), created: Some(time), parts,
            usage: None, system: None, parent_id: None, completed: true, finish: None, error: None }
    }
    fn text(id: &str, role: &str, time: u64) -> HistoryMessage {
        HistoryMessage { role: role.into(), text: "visible text".into(), time,
            message_id: Some(id.into()), part_id: Some(format!("{id}-text")), persona_id: None, local_only: false }
    }
    #[test]
    fn file_labels_survive_scoped_projection_without_binary_urls_or_synthetic_text() {
        let snapshot = vec![native("user", "user", 1, vec![
            part("pdf", "file", Some("b7.pdf"), false),
            part("pdf", "file", Some("duplicate.pdf"), false),
            part("hidden", "file", Some("generated.txt"), true),
            part("wrong", "tool", Some("tool.txt"), false),
            part("image", "file", Some("/private/fixture/photo.png"), false),
        ]), native("assistant", "assistant", 2, vec![part("artifact", "file", Some("output.txt"), false)])];
        let original = text("user", "user", 1);
        let before = serde_json::to_string(&original).unwrap();
        let files = collect(&snapshot);
        let result = serde_json::to_value(attach(vec![original.clone(), original, text("assistant", "assistant", 2)], files)).unwrap();
        assert_eq!(result[0]["attachments"], serde_json::json!([
            {"id":"pdf","name":"b7.pdf"}, {"id":"image","name":"photo.png"}
        ]));
        assert!(result[1].get("attachments").is_none());
        assert!(result[2].get("attachments").is_none());
        assert!(!before.contains("attachments"));
        assert!(!result.to_string().contains("/private"));
    }
    #[test]
    fn file_only_native_turns_remain_visible_and_legacy_local_notes_cannot_borrow_labels() {
        let files = collect(&[native("only", "user", 1, vec![part("p", "file", Some("only.pdf"), false)])]);
        let mut messages = vec![text("reply", "assistant", 2)];
        include_file_only_messages(&mut messages, &files);
        assert_eq!(messages[0].message_id.as_deref(), Some("only"));
        assert!(messages[0].text.is_empty());
        let result = serde_json::to_value(attach(messages, files)).unwrap();
        assert_eq!(result[0]["attachments"][0]["name"], "only.pdf");
        let mut local = text("only", "user", 1); local.local_only = true;
        let files = collect(&[native("only", "user", 1, vec![part("p", "file", Some("only.pdf"), false)])]);
        assert!(serde_json::to_value(attach(vec![local], files)).unwrap()[0].get("attachments").is_none());
    }
    #[test]
    fn absent_or_invalid_filename_does_not_create_an_empty_history_turn() {
        let snapshot = vec![native("u", "user", 1, vec![part("a", "file", None, false),
            part("b", "file", Some("/"), false), part("c", "file", Some(&"x".repeat(1025)), false)])];
        let files = collect(&snapshot);
        assert!(files.is_empty());
        let mut messages = vec![];
        include_file_only_messages(&mut messages, &files);
        assert!(messages.is_empty());
    }
}
