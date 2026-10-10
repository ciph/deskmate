//! Recover display identity from native metadata, never from conversational text.
use std::{collections::HashMap, fs, path::Path};
use crate::agent::NativeMessage;

struct ProfileCache {
    path: std::path::PathBuf,
    loaded: std::time::Instant,
    profiles: Vec<(String, String)>,
}
static PROFILES: std::sync::Mutex<Option<ProfileCache>> = std::sync::Mutex::new(None);

pub(crate) fn invalidate_profiles() {
    if let Ok(mut cache) = PROFILES.lock() { *cache = None; }
}

pub(super) fn cached_profiles(data: &Path) -> Vec<(String, String)> {
    let Ok(mut cache) = PROFILES.lock() else { return load_profiles(data); };
    if let Some(cached) = cache.as_ref().filter(|cached| cached.path == data && cached.loaded.elapsed().as_secs() < 30) {
        return cached.profiles.clone();
    }
    let profiles = load_profiles(data);
    *cache = Some(ProfileCache { path: data.into(), loaded: std::time::Instant::now(), profiles: profiles.clone() });
    profiles
}

fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Older plain Markdown personas have no ID. Match their full installed prompt,
/// with an exact section boundary; ambiguous matches intentionally remain unknown.
pub(super) fn load_profiles(data: &Path) -> Vec<(String, String)> {
    fn read(root: &Path, profiles: &mut Vec<(String, String)>) {
        let Ok(entries) = fs::read_dir(root) else { return; };
        for entry in entries.flatten() {
            let id = entry.file_name().to_string_lossy().into_owned();
            if !safe_id(&id) { continue; }
            let path = entry.path().join("persona.md");
            if fs::metadata(&path).is_ok_and(|m| m.len() <= 1024 * 1024) {
                if let Ok(prompt) = fs::read_to_string(path) {
                    let prompt = prompt.replace("\r\n", "\n").trim().to_owned();
                    if prompt.chars().count() >= 32 { profiles.push((id, prompt)); }
                }
            }
        }
    }
    let mut profiles = Vec::new();
    read(&data.join("personas"), &mut profiles);
    if let Ok(packs) = fs::read_dir(data.join("packs")) {
        for pack in packs.flatten() { read(&pack.path().join("personas"), &mut profiles); }
    }
    profiles
}

fn system_persona(system: &str, profiles: &[(String, String)]) -> Option<String> {
    let normalized = system.replace("\r\n", "\n");
    let system = normalized.trim();
    let first = system.lines().next()?;
    if first.starts_with("<!-- yume:persona=") {
        return first.strip_prefix("<!-- yume:persona=")?.strip_suffix(" -->")
            .filter(|id| safe_id(id)).map(str::to_owned);
    }
    if let Some(body) = system.strip_prefix("---\n") {
        if let Some((header, _)) = body.split_once("\n---\n") {
            let mut ids = header.lines().filter_map(|line| line.strip_prefix("id:"));
            if let Some(id) = ids.next() {
                let id = id.trim();
                return (safe_id(id) && ids.next().is_none()).then(|| id.to_owned());
            }
        }
    }
    let mut found: Option<&str> = None;
    for (id, prompt) in profiles {
        if system.strip_prefix(prompt.as_str()).is_some_and(|rest| rest.is_empty() || rest.starts_with("\n\n")) {
            if found.is_some_and(|previous| previous != id) { return None; }
            found = Some(id);
        }
    }
    found.map(str::to_owned)
}

pub(super) fn message_personas(messages: &[NativeMessage], profiles: &[(String, String)]) -> HashMap<String, String> {
    let users: HashMap<_, _> = messages.iter()
        .filter(|message| message.role.as_deref() == Some("user"))
        .filter_map(|message| Some((message.id.clone(), system_persona(message.system.as_deref()?, profiles)?)))
        .collect();
    let mut identities = users.clone();
    for message in messages.iter().filter(|message| message.role.as_deref() == Some("assistant")) {
        if let Some(id) = message.parent_id.as_ref().and_then(|parent| users.get(parent)) {
            identities.insert(message.id.clone(), id.clone());
        }
    }
    identities
}

#[cfg(test)]
mod tests {
    use super::*;
    fn message(id: &str, role: &str, parent: Option<&str>, system: Option<&str>) -> NativeMessage {
        NativeMessage { id: id.into(), role: Some(role.into()), parent_id: parent.map(str::to_owned),
            system: system.map(str::to_owned), created: Some(1), completed: true, finish: None, error: None, parts: vec![] }
    }
    #[test]
    fn mixed_turns_follow_exact_parent_even_out_of_order() {
        let messages = vec![
            message("a2", "assistant", Some("u2"), None),
            message("u1", "user", None, Some("---\nid: xiaoxiongchong\n---\n# Persona")),
            message("u2", "user", None, Some("<!-- yume:persona=xiaozhu-sandaime -->\n\nchanged prompt")),
            message("a1", "assistant", Some("u1"), None),
            message("a3", "assistant", Some("missing"), None),
            message("u3", "user", None, Some("You are helpful")),
            message("a4", "assistant", Some("u3"), Some("<!-- yume:persona=xiaozhu -->")),
        ];
        let ids = message_personas(&messages, &[]);
        assert_eq!(ids.get("a1").map(String::as_str), Some("xiaoxiongchong"));
        assert_eq!(ids.get("a2").map(String::as_str), Some("xiaozhu-sandaime"));
        assert!(!ids.contains_key("a3")); assert!(!ids.contains_key("a4"));
    }
    #[test]
    fn plain_imports_require_full_unambiguous_prompt_and_section_boundary() {
        let prompt = "# Bear\n\nA complete unique persona definition longer than thirty two characters.";
        let profiles = vec![("bear".into(), prompt.into())];
        assert_eq!(system_persona(&format!("{prompt}\n\n# Memories\n..."), &profiles).as_deref(), Some("bear"));
        assert_eq!(system_persona(&format!("{prompt} is quoted"), &profiles), None);
        assert_eq!(system_persona(&format!("# Context\n\n{prompt}"), &profiles), None);
        assert_eq!(system_persona(prompt, &[("bear".into(), prompt.into()), ("cat".into(), prompt.into())]), None);
    }
    #[test]
    fn ignores_mentions_nested_ids_and_invalid_metadata() {
        for prompt in ["我是小熊虫", "# Quote\n---\nid: xiaozhu\n---\ntext", "---\nid: ../bad\n---\ntext",
            "---\nid: bear\nid: cat\n---\ntext", "<!-- yume:persona=../bad -->\n\n---\nid: bear\n---\ntext"] {
            assert_eq!(system_persona(prompt, &[]), None);
        }
        assert_eq!(system_persona("---\r\nid: baobao\r\n---\r\ncat", &[]).as_deref(), Some("baobao"));
    }
    #[test]
    fn old_json_is_compatible_and_new_identity_survives_round_trip() {
        let mut message: crate::history::HistoryMessage = serde_json::from_str(r#"{"role":"assistant","text":"hi","time":1}"#).unwrap();
        assert_eq!(message.persona_id, None);
        message.persona_id = Some("baobao".into());
        let encoded = serde_json::to_string(&message).unwrap();
        assert!(encoded.contains("\"personaId\":\"baobao\""));
        assert_eq!(serde_json::from_str::<crate::history::HistoryMessage>(&encoded).unwrap(), message);
    }
}

#[test]
fn profile_cache_is_scoped_and_install_invalidation_reloads() {
    let root = std::env::temp_dir().join(format!("yume-profile-cache-{}", uuid::Uuid::new_v4()));
    let dir = root.join("personas/bear");
    std::fs::create_dir_all(&dir).unwrap();
    let prompt = "A sufficiently long synthetic profile for the bear persona.";
    std::fs::write(dir.join("persona.md"), prompt).unwrap();
    assert_eq!(cached_profiles(&root), vec![("bear".into(), prompt.into())]);
    std::fs::remove_file(dir.join("persona.md")).unwrap();
    assert_eq!(cached_profiles(&root).len(), 1);
    invalidate_profiles();
    assert!(cached_profiles(&root).is_empty());
    assert!(cached_profiles(&root.join("other-app")).is_empty());
    std::fs::remove_dir_all(root).unwrap();
}
