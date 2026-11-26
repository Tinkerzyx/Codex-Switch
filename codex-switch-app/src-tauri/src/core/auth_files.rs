use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub(crate) fn require_file_store(config: &str) -> Result<(), String> {
    let parsed: toml::Table = config
        .trim_start_matches('\u{feff}')
        .parse()
        .map_err(|e| format!("config.toml is incomplete or invalid: {e}"))?;
    let mode = parsed
        .get("cli_auth_credentials_store")
        .and_then(toml::Value::as_str)
        .unwrap_or("file");
    if mode != "file" {
        return Err(format!("Credential store '{mode}' is not supported by file-based profiles. Select file storage in Codex before switching; existing credentials were left untouched."));
    }
    Ok(())
}

pub(crate) fn validate(auth: &str, config: Option<&str>) -> Result<String, String> {
    if let Some(config) = config {
        require_file_store(config)?;
    }
    let parsed: Value = serde_json::from_str(auth.trim_start_matches('\u{feff}'))
        .map_err(|_| "Waiting for a complete auth.json.".to_string())?;
    let token = parsed["tokens"]["access_token"]
        .as_str()
        .filter(|v| !v.trim().is_empty());
    let api_key = parsed["OPENAI_API_KEY"]
        .as_str()
        .filter(|v| !v.trim().is_empty());
    if token.is_none() && api_key.is_none() {
        return Err("Waiting for Codex login credentials.".into());
    }
    let mut hash = Sha256::new();
    hash.update(auth.as_bytes());
    hash.update([0]);
    hash.update(if config.is_some() { [1] } else { [0] });
    hash.update(config.unwrap_or_default().as_bytes());
    Ok(format!("{:x}", hash.finalize()))
}

pub(crate) fn signature(directory: &Path) -> Result<String, String> {
    let config = match fs::read_to_string(directory.join("config.toml")) {
        Ok(raw) => Some(raw),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("Unable to read config.toml: {e}")),
    };
    if let Some(config) = config.as_deref() {
        require_file_store(config)?;
    }
    let auth = fs::read_to_string(directory.join("auth.json"))
        .map_err(|_| "Waiting for Codex to create auth.json.".to_string())?;
    validate(&auth, config.as_deref())
}

pub(crate) fn check_store(directory: &Path) -> Result<(), String> {
    match fs::read_to_string(directory.join("config.toml")) {
        Ok(config) => require_file_store(&config),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Unable to read config.toml: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn small_credentials_and_optional_config_are_supported() {
        assert!(validate(r#"{"tokens":{"access_token":"valid"}}"#, None).is_ok());
        assert!(validate(
            r#"{"tokens":{"access_token":"valid"}}"#,
            Some("model = 'gpt-5'\n")
        )
        .is_ok());
        assert!(validate(r#"{"OPENAI_API_KEY":"example"}"#, Some("")).is_ok());
        assert!(validate("{}", None).is_err());
        assert!(validate("{\"tokens\":", None).is_err());
        assert!(validate(r#"{"tokens":{"access_token":"valid"}}"#, Some("model = [")).is_err());
    }
    #[test]
    fn same_size_token_rotation_changes_the_stability_signature() {
        let first = validate(r#"{"tokens":{"access_token":"aaa"}}"#, None).unwrap();
        let second = validate(r#"{"tokens":{"access_token":"bbb"}}"#, None).unwrap();
        assert_ne!(first, second);
    }
    #[test]
    fn keyring_auto_and_ephemeral_are_detected_before_mutation() {
        for mode in ["keyring", "auto", "ephemeral"] {
            assert!(require_file_store(&format!("cli_auth_credentials_store = '{mode}'")).is_err());
        }
        assert!(require_file_store("cli_auth_credentials_store = 'file'").is_ok());
    }
}
