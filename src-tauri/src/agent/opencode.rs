use super::record_store::{NativeMessage, NativePart, NativeToolState};
use serde::{Deserialize, Serialize};
#[path = "opencode_settlement.rs"]
mod settlement;
#[cfg(test)]
#[path = "opencode_settlement_tests.rs"]
mod settlement_tests;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(crate) const READY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub(crate) struct AgentEndpoint {
    pub(crate) base_url: String,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) workspace: std::path::PathBuf,
    /// Basic-auth header value required by the managed sidecar.
    pub(crate) auth_header: String,
}
pub(crate) struct OpenCodeClient {
    endpoint: AgentEndpoint,
    agent: ureq::Agent,
}
#[derive(Deserialize)]
struct Session {
    id: String,
}
#[derive(Serialize)]
struct Model<'a> {
    #[serde(rename = "providerID")]
    provider_id: &'a str,
    #[serde(rename = "modelID")]
    model_id: &'a str,
}
#[derive(Deserialize)]
struct WireMessage {
    info: WireInfo,
    parts: Vec<WirePart>,
}
#[derive(Deserialize)]
struct WireInfo {
    system: Option<String>,
    id: String,
    role: Option<String>,
    #[serde(rename = "parentID")]
    parent_id: Option<String>,
    finish: Option<String>,
    time: WireTime,
    error: Option<WireError>,
    #[serde(rename = "providerID")]
    provider_id: Option<serde_json::Value>,
    #[serde(rename = "modelID")]
    model_id: Option<serde_json::Value>,
    tokens: Option<serde_json::Value>,
    #[serde(rename = "sessionID")]
    session_id: Option<serde_json::Value>,
}

impl WireInfo {
    fn usage(&self, session: &str) -> Option<crate::ai_usage::UsageObservation> {
        if self.role.as_deref() != Some("assistant") || self.time.completed.is_none()
            || self.session_id.as_ref().and_then(serde_json::Value::as_str) != Some(session)
        {
            return None;
        }
        let tokens = self.tokens.as_ref()?.as_object()?;
        let count = |field: &str| tokens.get(field).and_then(serde_json::Value::as_u64);
        let cache = tokens.get("cache");
        let cached = |field: &str| {
            cache.and_then(|cache| cache.get(field)).and_then(serde_json::Value::as_u64)
        };
        let observation = crate::ai_usage::UsageObservation {
            message_id: self.id.clone(),
            sidecar_id: self.provider_id.as_ref()?.as_str()?.to_owned(),
            model_id: self.model_id.as_ref()?.as_str()?.to_owned(),
            created_at_ms: i64::try_from(self.time.created?).ok()?,
            tokens: count("input")?
                .checked_add(count("output")?)?
                .checked_add(count("reasoning")?)?
                .checked_add(cached("read")?)?
                .checked_add(cached("write")?)?,
        };
        observation.is_valid().then_some(observation)
    }
}
#[derive(Deserialize)]
struct WireTime {
    created: Option<u64>,
    completed: Option<u64>,
}
#[derive(Deserialize)]
struct WireError {
    name: String,
}
#[derive(Deserialize)]
struct WirePart {
    filename: Option<String>,
    id: String,
    #[serde(default, rename = "messageID", deserialize_with = "present_identity")]
    message_id: Option<serde_json::Value>,
    #[serde(default, rename = "sessionID", deserialize_with = "present_identity")]
    session_id: Option<serde_json::Value>,
    #[serde(default)]
    synthetic: bool,
    #[serde(rename = "type")]
    kind: Option<String>,
    text: Option<String>,
    #[serde(rename = "callID")]
    call_id: Option<String>,
    tool: Option<String>,
    state: Option<WireToolState>,
}
// Preserve explicit null so a supplied but invalid part identity cannot pass as absent.
fn present_identity<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<serde_json::Value>, D::Error> {
    serde_json::Value::deserialize(deserializer).map(Some)
}

impl WireMessage {
    fn into_native(self, session: &str) -> NativeMessage {
        NativeMessage {
            usage: self.info.usage(session),
            system: self.info.system,
            id: self.info.id,
            role: self.info.role,
            created: self.info.time.created,
            parent_id: self.info.parent_id,
            completed: self.info.time.completed.is_some(),
            finish: self.info.finish,
            error: self.info.error.map(|error| error.name),
            parts: self.parts.into_iter().map(|part| NativePart {
                file_name: (part.kind.as_deref() == Some("file")).then_some(part.filename).flatten(),
                id: part.id,
                synthetic: part.synthetic,
                kind: part.kind,
                text: part.text,
                call_id: part.call_id,
                tool: part.tool,
                state: part.state.map(|state| NativeToolState {
                    input: state.input,
                    output: match (state.status.as_str(), state.error) {
                        ("error", Some(error)) => serde_json::Value::String(error),
                        _ => state.output,
                    },
                    metadata: state.metadata,
                    status: state.status,
                }),
            }).collect(),
        }
    }
}

#[derive(Deserialize)]
struct WireToolState {
    status: String,
    #[serde(default)]
    input: serde_json::Value,
    #[serde(default)]
    output: serde_json::Value,
    error: Option<String>,
    #[serde(default)]
    metadata: serde_json::Value,
}

impl OpenCodeClient {
    pub(crate) fn new(endpoint: AgentEndpoint) -> Self {
        Self {
            endpoint,
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(15))
                .build(),
        }
    }
    pub(crate) fn create_session(&self, workspace: &Path) -> Result<String, String> {
        debug_assert_eq!(workspace, self.endpoint.workspace);
        let session: Session = self
            .agent
            .post(&self.workspace_url("/session")?)
            .set("Authorization", &self.endpoint.auth_header)
            .send_json(serde_json::json!({
                "title":"YUME Agent",
                "permission":[
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
                ]
            }))
            .map_err(http_error)?
            .into_json()
            .map_err(|_| "agent_wire_invalid")?;
        if session.id.is_empty() {
            return Err("agent_wire_invalid".into());
        }
        Ok(session.id)
    }
    pub(crate) fn wait_ready(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self
                .agent
                .get(&format!("{}/global/health", self.endpoint.base_url))
                .set("Authorization", &self.endpoint.auth_header)
                .timeout(Duration::from_millis(500))
                .call()
                .is_ok()
            {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        false
    }
    pub(crate) fn prompt(
        &self,
        session: &str,
        message_id: &str,
        system: &str,
        input: &str,
    ) -> Result<(), String> {
        self.prompt_with_parts(session, message_id, system, input, &[])
    }
    pub(crate) fn prompt_with_parts(
        &self,
        session: &str,
        message_id: &str,
        system: &str,
        input: &str,
        resource_parts: &[serde_json::Value],
    ) -> Result<(), String> {
        let mut parts = vec![serde_json::json!({"type":"text","text":input})];
        parts.extend_from_slice(resource_parts);
        self.agent.post(&self.workspace_url(&format!("/session/{session}/prompt_async"))?)
            .set("Authorization", &self.endpoint.auth_header)
            .send_json(serde_json::json!({"messageID":message_id,"model":Model { provider_id:&self.endpoint.provider_id, model_id:&self.endpoint.model_id },"system":system,"parts":parts}))
            .map_err(http_error)?;
        Ok(())
    }
    pub(crate) fn abort(&self, session: &str) -> Result<bool, String> {
        self.agent
            .post(&self.workspace_url(&format!("/session/{session}/abort"))?)
            .set("Authorization", &self.endpoint.auth_header)
            .call()
            .map_err(http_error)?
            .into_json()
            .map_err(|_| "agent_wire_invalid".into())
    }
    pub(crate) fn snapshot(&self, session: &str) -> Result<Vec<NativeMessage>, String> {
        let messages: Vec<WireMessage> = self
            .agent
            .get(&self.workspace_url(&format!("/session/{session}/message"))?)
            .set("Authorization", &self.endpoint.auth_header)
            .call()
            .map_err(http_error)?
            .into_json()
            .map_err(|_| "agent_wire_invalid")?;
        if messages.iter().any(|message| {
            message.info.session_id.as_ref().is_some_and(|id| id.as_str() != Some(session))
        }) {
            return Err("agent_snapshot_session_mismatch".into());
        }
        Ok(messages.into_iter().map(|message| message.into_native(session)).collect())
    }

    pub(crate) fn message(&self, session: &str, message_id: &str) -> Result<NativeMessage, String> {
        if !crate::worklog::bridge::safe_id(session) || !crate::worklog::bridge::safe_id(message_id) {
            return Err("agent_invalid_id".into());
        }
        let message: WireMessage = self.agent
            .get(&self.workspace_url(&format!("/session/{session}/message/{message_id}"))?)
            .set("Authorization", &self.endpoint.auth_header)
            .call()
            .map_err(http_error)?
            .into_json()
            .map_err(|_| "agent_wire_invalid")?;
        if message.info.id != message_id
            || message.info.session_id.as_ref().and_then(serde_json::Value::as_str) != Some(session)
            || message.parts.iter().any(|part| {
                part.message_id.as_ref().is_some_and(|id| id.as_str() != Some(message_id))
                    || part.session_id.as_ref().is_some_and(|id| id.as_str() != Some(session))
            })
        {
            return Err("agent_message_identity_mismatch".into());
        }
        if !matches!(message.info.role.as_deref(), Some("user" | "assistant"))
            || message.info.time.created.is_none()
            || (message.info.role.as_deref() == Some("assistant")
                && message.info.parent_id.as_deref().is_none_or(|id| !crate::worklog::bridge::safe_id(id)))
        {
            return Err("agent_message_incomplete".into());
        }
        let mut part_ids = std::collections::HashSet::new();
        for part in &message.parts {
            if part.id.is_empty() || !part_ids.insert(&part.id)
                || part.kind.as_deref().is_none_or(str::is_empty)
                || part.message_id.is_none() || part.session_id.is_none()
                || (part.kind.as_deref() == Some("tool")
                    && (part.call_id.as_deref().is_none_or(str::is_empty)
                        || part.tool.as_deref().is_none_or(str::is_empty)
                        || part.state.as_ref().is_none_or(|state| {
                            !matches!(state.status.as_str(), "pending" | "running" | "completed" | "error")
                        })))
                || (matches!(part.kind.as_deref(), Some("text" | "reasoning")) && part.text.is_none())
            {
                return Err("agent_message_incomplete".into());
            }
        }
        Ok(message.into_native(session))
    }

    fn workspace_url(&self, path: &str) -> Result<String, String> {
        let mut url = url::Url::parse(&format!("{}{path}", self.endpoint.base_url))
            .map_err(|_| "agent_endpoint_invalid".to_owned())?;
        url.query_pairs_mut().append_pair(
            "directory",
            &super::opencode_wire_directory(&self.endpoint.workspace),
        );
        Ok(url.into())
    }
}
fn http_error(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(status, _) => format!("agent_http_{status}"),
        ureq::Error::Transport(_) => "agent_transport_failure".into(),
    }
}
