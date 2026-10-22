use base64::Engine;
use tauri::{http::HeaderMap, ipc::InvokeBody};

use super::types::SessionId;
use super::types::StageChatAttachmentRequest;
use super::validation::{parse_mime, sanitize_filename};

const HEADER: &str = "x-yume-upload";
const MAX_HEADER_BYTES: usize = 8192;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct UploadMetadata {
    pub upload_id: String,
    pub scope_key: String,
    pub session_id: Option<String>,
    pub file_name: String,
    pub mime: String,
    pub size: usize,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BinaryUploadReceipt<T> {
    upload_id: String,
    scope_key: String,
    value: T,
}

impl UploadMetadata {
    pub fn receipt<T>(&self, value: T) -> BinaryUploadReceipt<T> {
        BinaryUploadReceipt {
            upload_id: self.upload_id.clone(),
            scope_key: self.scope_key.clone(),
            value,
        }
    }

    pub fn stage_request(&self, bytes: Vec<u8>) -> StageChatAttachmentRequest {
        StageChatAttachmentRequest {
            session_id: self
                .session_id
                .clone()
                .expect("validated attachment session"),
            file_name: self.file_name.clone(),
            mime: self.mime.clone(),
            size: self.size,
            bytes,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum UploadKind {
    Attachment,
    Media,
}

// Scope/upload IDs correlate a response with a frontend generation, not filesystem authority.
pub(super) fn parse<'a>(
    body: &'a InvokeBody,
    headers: &HeaderMap,
    kind: UploadKind,
) -> Result<(UploadMetadata, &'a [u8]), String> {
    let InvokeBody::Raw(bytes) = body else {
        return Err("upload_binary_body_required".into());
    };
    let mut values = headers.get_all(HEADER).iter();
    let encoded = values.next().ok_or("upload_metadata_required")?;
    if values.next().is_some() || encoded.as_bytes().len() > MAX_HEADER_BYTES {
        return Err("upload_metadata_invalid".into());
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .map_err(|_| "upload_metadata_invalid")?;
    let metadata: UploadMetadata =
        serde_json::from_slice(&decoded).map_err(|_| "upload_metadata_invalid")?;
    if metadata.upload_id.is_empty()
        || metadata.upload_id.len() > 128
        || !metadata
            .upload_id
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || b"._:-".contains(&ch))
        || metadata.scope_key.trim().is_empty()
        || metadata.scope_key.len() > 1024
        || metadata.scope_key.chars().any(char::is_control)
        || metadata.file_name.chars().count() > 255
        || metadata.mime.len() > 127
    {
        return Err("upload_metadata_invalid".into());
    }
    sanitize_filename(&metadata.file_name).map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("attachment data must not be empty".into());
    }
    if metadata.size != bytes.len() {
        return Err("attachment size does not match data".into());
    }
    let limit_mib = match kind {
        UploadKind::Attachment => {
            let session = metadata
                .session_id
                .as_ref()
                .ok_or("invalid attachment session")?;
            if session.len() > 1024 {
                return Err("invalid attachment session".into());
            }
            SessionId::parse(session.clone()).map_err(|e| e.to_string())?;
            parse_mime(&metadata.mime).map_err(|e| e.to_string())?;
            if metadata.mime == "application/x-ncm" {
                64
            } else {
                20
            }
        }
        UploadKind::Media => {
            if metadata.session_id.is_some() {
                return Err("upload_metadata_invalid".into());
            }
            super::resources::validate_media_upload(
                &metadata.file_name,
                &metadata.mime,
                bytes.len(),
            )?;
            64
        }
    };
    if bytes.len() > limit_mib * 1024 * 1024 {
        return Err(format!("attachment item exceeds {limit_mib} MiB"));
    }
    if matches!(kind, UploadKind::Attachment) {
        validate_image_signature(&metadata.mime, bytes)?;
    }
    Ok((metadata, bytes))
}

fn validate_image_signature(mime: &str, bytes: &[u8]) -> Result<(), String> {
    // These signatures reject mislabeled inputs; the image decoder still validates full content.
    let valid = match mime {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err("attachment image signature does not match mime".into())
    }
}

#[cfg(test)]
mod tests;
