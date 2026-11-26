use reqwest::{blocking::Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const EMBEDDED_CONFIG: &str = include_str!("../cloudbase.config.json");

#[derive(Serialize, Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
pub struct CloudBaseConfig {
    pub env_id: String,
    pub client_function: String,
    pub admin_function: String,
}

impl Default for CloudBaseConfig {
    fn default() -> Self {
        Self {
            env_id: String::new(),
            client_function: "csw_client_api".to_string(),
            admin_function: "csw_admin_api".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct CloudBaseSession {
    device_id: String,
    token_type: String,
    access_token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudBaseStatus {
    pub configured: bool,
    pub env_id: String,
    pub client_function: String,
    pub admin_function: String,
    pub config_path: String,
}

#[derive(Clone, Copy)]
pub enum FunctionKind {
    Client,
    Admin,
}

fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("cloudbase.json")
}

fn session_path(data_dir: &Path) -> PathBuf {
    data_dir.join("cloudbase_session.json")
}

fn normalize_config(mut config: CloudBaseConfig) -> CloudBaseConfig {
    config.env_id = config.env_id.trim().to_string();
    config.client_function = config.client_function.trim().to_string();
    config.admin_function = config.admin_function.trim().to_string();
    if config.client_function.is_empty() {
        config.client_function = "csw_client_api".to_string();
    }
    if config.admin_function.is_empty() {
        config.admin_function = "csw_admin_api".to_string();
    }
    config
}

fn load_config(data_dir: &Path) -> Result<CloudBaseConfig, String> {
    let override_path = config_path(data_dir);
    let raw = if override_path.exists() {
        fs::read_to_string(&override_path).map_err(|error| error.to_string())?
    } else {
        EMBEDDED_CONFIG.to_string()
    };
    let config = serde_json::from_str::<CloudBaseConfig>(raw.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("Failed to parse CloudBase config: {error}"))?;
    Ok(normalize_config(config))
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    let raw = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    fs::write(&tmp, raw).map_err(|error| error.to_string())?;
    fs::rename(&tmp, path)
        .or_else(|_| {
            fs::copy(&tmp, path)
                .map(|_| ())
                .and_then(|_| fs::remove_file(&tmp))
        })
        .map_err(|error| error.to_string())
}

fn load_session(data_dir: &Path) -> Result<CloudBaseSession, String> {
    let path = session_path(data_dir);
    if !path.exists() {
        return Ok(CloudBaseSession {
            device_id: format!("csw_{}", Uuid::new_v4().simple()),
            ..CloudBaseSession::default()
        });
    }
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut session = serde_json::from_str::<CloudBaseSession>(raw.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("Failed to parse CloudBase session: {error}"))?;
    if session.device_id.trim().is_empty() {
        session.device_id = format!("csw_{}", Uuid::new_v4().simple());
    }
    Ok(session)
}

fn save_session(data_dir: &Path, session: &CloudBaseSession) -> Result<(), String> {
    write_json_atomic(&session_path(data_dir), session)
}

fn client() -> Result<Client, String> {
    crate::services::http::client()
}

fn gateway_base(config: &CloudBaseConfig) -> String {
    format!("https://{}.api.tcloudbasegateway.com", config.env_id)
}

fn sign_in_anonymously(
    client: &Client,
    config: &CloudBaseConfig,
    data_dir: &Path,
    session: &mut CloudBaseSession,
) -> Result<(), String> {
    let url = format!("{}/auth/v1/signin/anonymously", gateway_base(config));
    let response = client
        .post(url)
        .header("x-device-id", &session.device_id)
        .send()
        .map_err(|error| format!("CloudBase anonymous login failed: {error}"))?;
    let status = response.status();
    let value = response
        .json::<Value>()
        .map_err(|error| format!("CloudBase login returned invalid JSON: {error}"))?;
    if !status.is_success() {
        return Err(error_message(
            &value,
            "CloudBase anonymous login was rejected.",
        ));
    }
    session.token_type = value
        .get("token_type")
        .and_then(Value::as_str)
        .unwrap_or("Bearer")
        .to_string();
    session.access_token = value
        .get("access_token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if session.access_token.is_empty() {
        return Err("CloudBase anonymous login did not return an access token.".to_string());
    }
    save_session(data_dir, session)
}

fn error_message(value: &Value, fallback: &str) -> String {
    value
        .get("message")
        .or_else(|| value.get("msg"))
        .or_else(|| value.get("error_description"))
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn unwrap_function_value(mut value: Value) -> Value {
    for _ in 0..3 {
        if let Some(text) = value.get("response_data").and_then(Value::as_str) {
            if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                value = parsed;
                continue;
            }
        }
        if let Some(text) = value.get("body").and_then(Value::as_str) {
            if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                value = parsed;
                continue;
            }
        }
        if let Some(data) = value.get("data") {
            if data.is_object()
                && (data.get("success").is_some() || data.get("response_data").is_some())
            {
                value = data.clone();
                continue;
            }
        }
        break;
    }
    value
}

fn invoke_once(
    client: &Client,
    config: &CloudBaseConfig,
    session: &CloudBaseSession,
    function_name: &str,
    payload: &Value,
) -> Result<(StatusCode, Value), String> {
    let token_type = if session.token_type.trim().is_empty() {
        "Bearer"
    } else {
        session.token_type.trim()
    };
    let url = format!("{}/v1/functions/{}", gateway_base(config), function_name);
    let response = client
        .post(url)
        .header(
            "Authorization",
            format!("{token_type} {}", session.access_token),
        )
        .header("Accept", "application/json")
        .json(payload)
        .send()
        .map_err(|error| format!("CloudBase function request failed: {error}"))?;
    let status = response.status();
    let text = response.text().map_err(|error| error.to_string())?;
    let value = serde_json::from_str::<Value>(&text)
        .map(unwrap_function_value)
        .map_err(|error| {
            format!(
                "CloudBase function returned invalid JSON: {error}; body: {}",
                text.chars().take(240).collect::<String>()
            )
        })?;
    Ok((status, value))
}

pub fn status(data_dir: &Path) -> Result<CloudBaseStatus, String> {
    let config = load_config(data_dir)?;
    Ok(CloudBaseStatus {
        configured: !config.env_id.is_empty(),
        env_id: config.env_id,
        client_function: config.client_function,
        admin_function: config.admin_function,
        config_path: config_path(data_dir).display().to_string(),
    })
}

pub fn invoke(data_dir: &Path, kind: FunctionKind, payload: Value) -> Result<Value, String> {
    let config = load_config(data_dir)?;
    if config.env_id.is_empty() {
        return Err(format!(
            "CloudBase environment is not configured. Set envId in {} or src-tauri/cloudbase.config.json.",
            config_path(data_dir).display()
        ));
    }
    let function_name = match kind {
        FunctionKind::Client => &config.client_function,
        FunctionKind::Admin => &config.admin_function,
    };
    let client = client()?;
    let mut session = load_session(data_dir)?;
    if session.access_token.trim().is_empty() {
        sign_in_anonymously(&client, &config, data_dir, &mut session)?;
    }

    let (mut status, mut value) = invoke_once(&client, &config, &session, function_name, &payload)?;
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        session.access_token.clear();
        session.token_type.clear();
        sign_in_anonymously(&client, &config, data_dir, &mut session)?;
        (status, value) = invoke_once(&client, &config, &session, function_name, &payload)?;
    }
    if !status.is_success() {
        return Err(error_message(
            &value,
            "CloudBase function request was rejected.",
        ));
    }
    if value.get("success").and_then(Value::as_bool) == Some(false) {
        return Err(error_message(&value, "CloudBase operation failed."));
    }
    Ok(value)
}

pub fn installation_id(data_dir: &Path) -> Result<String, String> {
    let session = load_session(data_dir)?;
    save_session(data_dir, &session)?;
    Ok(session.device_id)
}
