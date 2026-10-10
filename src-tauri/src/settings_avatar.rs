use base64::Engine;

pub(super) fn validate(value: Option<&str>) -> Result<(), String> {
    let Some(value) = value else { return Ok(()) };
    let invalid = || "Invalid avatar: expected a PNG thumbnail up to 128 × 128".to_string();
    if value.len() > 96 * 1024 {
        return Err(invalid());
    }
    let encoded = value
        .strip_prefix("data:image/png;base64,")
        .ok_or_else(invalid)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| invalid())?;
    // Bound decoded dimensions before the WebView loads the locally stored image.
    if bytes.len() < 33 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[8..16] != b"\0\0\0\rIHDR" {
        return Err(invalid());
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().map_err(|_| invalid())?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().map_err(|_| invalid())?);
    if width == 0 || width != height || width > 128 {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::Settings;
    use super::*;
    const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1cAAAAASUVORK5CYII=";

    #[test]
    fn legacy_default_and_avatar_round_trip() {
        let mut settings: Settings =
            serde_json::from_str(r#"{"theme":"mint","userName":"Keep"}"#).unwrap();
        assert_eq!(settings.user_avatar, None);
        assert!(validate(None).is_ok());
        assert!(validate(Some(PNG)).is_ok());
        settings.user_avatar = Some(PNG.into());
        let value = serde_json::to_value(settings).unwrap();
        assert_eq!(value["userAvatar"], PNG);
        let restored: Settings = serde_json::from_value(value).unwrap();
        assert_eq!(restored.theme, "mint");
        assert_eq!(restored.user_name, "Keep");
        assert_eq!(restored.user_avatar.as_deref(), Some(PNG));
    }

    #[test]
    fn rejects_external_invalid_and_oversized_images() {
        for value in [
            "",
            "https://example.org/avatar.png",
            "data:image/svg+xml;base64,AAAA",
            "data:image/png;base64,AAAA",
            "data:image/png;base64,!",
        ] {
            assert!(validate(Some(value)).is_err(), "{value}");
        }
        assert!(validate(Some(&"x".repeat(96 * 1024 + 1))).is_err());
        let mut bytes = base64::engine::general_purpose::STANDARD
            .decode(PNG.split(',').nth(1).unwrap())
            .unwrap();
        for dimension in [0_u32, 129, u32::MAX] {
            bytes[16..20].copy_from_slice(&dimension.to_be_bytes());
            let value = format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&bytes)
            );
            assert!(validate(Some(&value)).is_err());
        }
    }
}
