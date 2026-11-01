use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorklogOutcome {
    message_id: String,
    request_id: String,
    error: Option<String>,
    undoable: bool,
}

fn outcome(message_id: &str, tool: &str, output: &Value) -> Option<WorklogOutcome> {
    if !matches!(
        tool,
        "worklog_record" | "worklog_update" | "worklog_generate_report" | "worklog_schedule_report"
    ) {
        return None;
    }
    let decoded;
    let value = if let Some(text) = output.as_str() {
        decoded = serde_json::from_str::<Value>(text).ok()?;
        &decoded
    } else {
        output
    };
    if value.get("status").and_then(Value::as_str) == Some("completed")
        && value
            .pointer("/result/alreadyRecorded")
            .and_then(Value::as_bool)
            == Some(true)
    {
        return None;
    }
    let request_id = value.get("requestId")?.as_str()?;
    if request_id.len() != 36 || uuid::Uuid::parse_str(request_id).is_err() {
        return None;
    }
    let error = value
        .pointer("/error/code")
        .and_then(Value::as_str)
        .filter(|code| {
            !code.is_empty()
                && code.len() <= 64
                && code.bytes().all(|c| c.is_ascii_uppercase() || c == b'_')
        })
        .map(str::to_owned);
    Some(WorklogOutcome {
        message_id: message_id.into(),
        request_id: request_id.into(),
        error,
        undoable: tool == "worklog_record",
    })
}

pub(crate) fn project(messages: &[crate::agent::NativeMessage]) -> Vec<WorklogOutcome> {
    messages
        .iter()
        .flat_map(|message| {
            message.parts.iter().filter_map(|part| {
                if part.kind.as_deref() != Some("tool") {
                    return None;
                }
                outcome(
                    &message.id,
                    part.tool.as_deref()?,
                    &part.state.as_ref()?.output,
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn projects_only_valid_tool_identity_and_explicit_outcome() {
        let value = json!({"requestId":"ae0e8d01-25f2-2d08-3217-8d2de99cbedf", "status":"rejected", "error":{"code":"NEEDS_EXPLICIT_REQUEST"}, "secret":"must not project"});
        let result = outcome("message", "worklog_update", &value).unwrap();
        assert_eq!(result.error.as_deref(), Some("NEEDS_EXPLICIT_REQUEST"));
        assert!(!serde_json::to_string(&result).unwrap().contains("secret"));
        assert!(outcome("message", "worklog_query", &value).is_none());
        assert!(outcome("message", "shell", &value).is_none());
        assert!(outcome("message", "worklog_record", &json!({"requestId":"invalid"})).is_none());
        let mut duplicate = value.clone();
        duplicate["status"] = json!("completed");
        duplicate["result"] = json!({"alreadyRecorded":true});
        assert!(outcome("message", "worklog_record", &duplicate).is_none());
        assert!(
            outcome("message", "worklog_record", &json!(value.to_string()))
                .unwrap()
                .undoable
        );
    }
}
