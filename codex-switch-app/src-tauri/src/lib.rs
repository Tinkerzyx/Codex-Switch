use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Emitter, LogicalSize, Manager, State,
};

const TRAY_ID: &str = "codex-switch-tray";
const APP_ICON: Image<'_> = tauri::include_image!("icons/codex-switch.ico");
const AUTH_FILE: &str = "auth.json";
const CONFIG_FILE: &str = "config.toml";
const MIN_AUTH_BYTES: u64 = 3 * 1024;
const MIN_CONFIG_BYTES: u64 = 2 * 1024;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Default)]
struct RuntimeState {
    launch_target: Mutex<Option<ShellApp>>,
    tray_profiles: Mutex<Vec<TrayProfile>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandResult {
    ok: bool,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSnapshot {
    active: Option<String>,
    codex_dir: String,
    data_dir: String,
    launch_target: Option<String>,
    profiles: Vec<ProfileView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileView {
    name: String,
    active: bool,
    plan: String,
    email: String,
    account_id: String,
    primary_left: Option<f64>,
    secondary_left: Option<f64>,
    reset: String,
    updated: String,
    auth_size: String,
    config_size: String,
    auth_text: String,
    config_text: String,
}

#[derive(Serialize, Deserialize, Default)]
struct Db {
    active: Option<String>,
    profiles: BTreeMap<String, ProfileMeta>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProfileMeta {
    name: String,
    created_at: String,
    updated_at: String,
}

#[derive(Deserialize, Clone)]
struct TrayProfile {
    name: String,
    active: bool,
}

#[derive(Serialize, Clone)]
struct TrayMenuPayload {
    action: String,
    index: Option<usize>,
}

#[derive(Serialize, Clone)]
struct PanelPayload {
    title: String,
    body_html: String,
}

#[derive(Clone)]
struct ShellApp {
    name: String,
    target: String,
}

#[derive(Default)]
struct SessionInfo {
    name: Option<String>,
    email: Option<String>,
    account_id: Option<String>,
    auth_mode: Option<String>,
    last_refresh: Option<String>,
    access_token: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginState {
    ready: bool,
    signature: Option<String>,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileUsage {
    profile_name: String,
    plan: String,
    primary_left: Option<f64>,
    secondary_left: Option<f64>,
    reset: String,
}

impl CommandResult {
    fn ok(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            message: message.into(),
        }
    }
}

fn now_stamp() -> String {
    let ps = r#"(Get-Date).ToString('yyyy-MM-dd_HHmmss')"#;
    run_powershell(ps)
        .unwrap_or_else(|_| {
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            secs.to_string()
        })
        .trim()
        .to_string()
}

fn now_text() -> String {
    let ps = r#"(Get-Date).ToString('yyyy-MM-dd HH:mm:ss')"#;
    run_powershell(ps)
        .unwrap_or_else(|_| now_stamp())
        .trim()
        .to_string()
}

fn run_powershell(script: &str) -> Result<String, String> {
    let mut command = Command::new("powershell.exe");
    command.args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        script,
    ]);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let output = command.output().map_err(|error| error.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else if stderr.is_empty() {
        Err(stdout)
    } else {
        Err(stderr)
    }
}

fn codex_dir() -> PathBuf {
    if let Ok(home) = env::var("CODEX_HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string())).join(".codex")
}

fn locate_data_dir() -> PathBuf {
    if let Ok(current) = env::current_dir() {
        for dir in current.ancestors() {
            let candidate = dir.join("codex_switch_data");
            if candidate.exists() {
                return candidate;
            }
        }
        if let Some(parent) = current.parent().and_then(Path::parent) {
            return parent.join("codex_switch_data");
        }
        return current.join("codex_switch_data");
    }
    PathBuf::from("codex_switch_data")
}

fn profiles_dir() -> PathBuf {
    locate_data_dir().join("profiles")
}

fn backups_dir() -> PathBuf {
    locate_data_dir().join("backups")
}

fn trash_dir() -> PathBuf {
    locate_data_dir().join("trash")
}

fn db_path() -> PathBuf {
    locate_data_dir().join("profiles.json")
}

fn ensure_dirs() -> Result<(), String> {
    fs::create_dir_all(profiles_dir()).map_err(|error| error.to_string())?;
    fs::create_dir_all(backups_dir()).map_err(|error| error.to_string())?;
    fs::create_dir_all(trash_dir()).map_err(|error| error.to_string())?;
    Ok(())
}

fn load_db() -> Result<Db, String> {
    ensure_dirs()?;
    let path = db_path();
    if !path.exists() {
        let db = Db::default();
        save_db(&db)?;
        return Ok(db);
    }
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let normalized = raw.trim_start_matches('\u{feff}');
    serde_json::from_str::<Db>(normalized)
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))
}

fn save_db(db: &Db) -> Result<(), String> {
    ensure_dirs()?;
    let path = db_path();
    let tmp = path.with_extension("json.tmp");
    let raw = serde_json::to_string_pretty(db).map_err(|error| error.to_string())?;
    fs::write(&tmp, raw).map_err(|error| error.to_string())?;
    fs::rename(&tmp, &path)
        .or_else(|_| {
            fs::copy(&tmp, &path)
                .map(|_| ())
                .and_then(|_| fs::remove_file(&tmp))
        })
        .map_err(|error| error.to_string())
}

fn current_file(name: &str) -> PathBuf {
    codex_dir().join(name)
}

fn profile_path(name: &str) -> PathBuf {
    profiles_dir().join(name)
}

fn managed_files() -> [&'static str; 2] {
    [AUTH_FILE, CONFIG_FILE]
}

fn current_files_complete() -> bool {
    managed_files()
        .iter()
        .all(|name| current_file(name).exists())
}

fn profile_files_complete(name: &str) -> bool {
    managed_files()
        .iter()
        .all(|file| profile_path(name).join(file).exists())
}

fn file_size_label(path: &Path) -> String {
    match fs::metadata(path) {
        Ok(meta) => {
            let bytes = meta.len();
            if bytes >= 1024 {
                format!("{:.1} KB", bytes as f64 / 1024.0)
            } else {
                format!("{bytes} B")
            }
        }
        Err(_) => "Missing".to_string(),
    }
}

fn read_text(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|_| format!("{} is missing.", path.display()))
}

fn backup_current(reason: &str, move_files: bool) -> Result<Option<PathBuf>, String> {
    ensure_dirs()?;
    let existing: Vec<PathBuf> = managed_files()
        .iter()
        .map(|name| current_file(name))
        .filter(|path| path.exists())
        .collect();
    if existing.is_empty() {
        return Ok(None);
    }

    let dir = backups_dir().join(format!("{}_{}", now_stamp(), reason));
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    for src in existing {
        let dest = dir.join(src.file_name().unwrap_or_default());
        if move_files {
            fs::rename(&src, &dest)
                .or_else(|_| {
                    fs::copy(&src, &dest)
                        .map(|_| ())
                        .and_then(|_| fs::remove_file(&src))
                })
                .map_err(|error| error.to_string())?;
        } else {
            fs::copy(&src, &dest).map_err(|error| error.to_string())?;
        }
    }
    Ok(Some(dir))
}

fn save_current_to_profile(profile_name: &str) -> Result<(), String> {
    if !current_files_complete() {
        return Err(format!(
            "{} must contain both auth.json and config.toml.",
            codex_dir().display()
        ));
    }

    let dest = profile_path(profile_name);
    fs::create_dir_all(&dest).map_err(|error| error.to_string())?;
    for name in managed_files() {
        fs::copy(current_file(name), dest.join(name)).map_err(|error| error.to_string())?;
    }

    let mut db = load_db()?;
    let created_at = db
        .profiles
        .get(profile_name)
        .map(|item| item.created_at.clone())
        .unwrap_or_else(now_text);
    db.profiles.insert(
        profile_name.to_string(),
        ProfileMeta {
            name: profile_name.to_string(),
            created_at,
            updated_at: now_text(),
        },
    );
    db.active = Some(profile_name.to_string());
    save_db(&db)
}

fn restore_profile(profile_name: &str) -> Result<(), String> {
    if !profile_files_complete(profile_name) {
        return Err(
            "Selected profile is incomplete. Missing auth.json or config.toml.".to_string(),
        );
    }

    fs::create_dir_all(codex_dir()).map_err(|error| error.to_string())?;
    for name in managed_files() {
        let path = current_file(name);
        if path.exists() {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }
    for name in managed_files() {
        let tmp = current_file(&format!("{name}.tmp"));
        fs::copy(profile_path(profile_name).join(name), &tmp).map_err(|error| error.to_string())?;
        fs::rename(&tmp, current_file(name))
            .or_else(|_| {
                fs::copy(&tmp, current_file(name))
                    .map(|_| ())
                    .and_then(|_| fs::remove_file(&tmp))
            })
            .map_err(|error| error.to_string())?;
    }

    let mut db = load_db()?;
    db.active = Some(profile_name.to_string());
    save_db(&db)
}

fn safe_profile_name(name: &str) -> String {
    let invalid = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut clean = name
        .trim()
        .chars()
        .map(|ch| if invalid.contains(&ch) { '_' } else { ch })
        .collect::<String>();
    clean = clean.trim_matches([' ', '.']).to_string();
    if clean.is_empty() {
        "Codex_Profile".to_string()
    } else {
        clean
    }
}

fn unique_profile_name(base: &str) -> Result<String, String> {
    let db = load_db()?;
    let clean = safe_profile_name(base);
    if !db.profiles.contains_key(&clean) {
        return Ok(clean);
    }
    let stamped = format!("{}_{}", clean, now_stamp());
    if !db.profiles.contains_key(&stamped) {
        return Ok(stamped);
    }
    let mut index = 2;
    loop {
        let candidate = format!("{stamped}_{index}");
        if !db.profiles.contains_key(&candidate) {
            return Ok(candidate);
        }
        index += 1;
    }
}

fn decode_base64_url(input: &str) -> Option<Vec<u8>> {
    let mut bits: u32 = 0;
    let mut bit_count = 0;
    let mut out = Vec::new();
    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            b'=' => break,
            _ => return None,
        } as u32;
        bits = (bits << 6) | value;
        bit_count += 6;
        if bit_count >= 8 {
            bit_count -= 8;
            out.push(((bits >> bit_count) & 0xff) as u8);
        }
    }
    Some(out)
}

fn decode_jwt_payload(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = decode_base64_url(payload)?;
    serde_json::from_slice(&bytes).ok()
}

fn value_string(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(ToString::to_string)
}

fn session_from_auth(path: &Path) -> SessionInfo {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(_) => return SessionInfo::default(),
    };
    let auth: Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(_) => return SessionInfo::default(),
    };
    let tokens = auth.get("tokens").unwrap_or(&Value::Null);
    let id_token = tokens
        .get("id_token")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let access_token = tokens
        .get("access_token")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let token_account_id = tokens
        .get("account_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let id_payload = decode_jwt_payload(id_token).unwrap_or(Value::Null);
    let access_payload = decode_jwt_payload(access_token).unwrap_or(Value::Null);
    let jwt_account_id = access_payload
        .get("https://api.openai.com/auth")
        .and_then(|item| item.get("chatgpt_account_id"))
        .and_then(Value::as_str)
        .map(ToString::to_string);

    SessionInfo {
        name: value_string(&id_payload, "name"),
        email: value_string(&id_payload, "email"),
        account_id: jwt_account_id.or(token_account_id),
        auth_mode: value_string(&auth, "auth_mode"),
        last_refresh: value_string(&auth, "last_refresh"),
        access_token: if access_token.is_empty() {
            None
        } else {
            Some(access_token.to_string())
        },
    }
}

fn usage_for_profile(profile_name: &str) -> Result<ProfileUsage, String> {
    let auth_path = profile_path(profile_name).join(AUTH_FILE);
    let session = session_from_auth(&auth_path);
    let access_token = session
        .access_token
        .ok_or_else(|| "auth.json does not contain tokens.access_token.".to_string())?;
    let account_line = session
        .account_id
        .as_ref()
        .map(|account_id| {
            format!(
                "$headers['ChatGPT-Account-Id']='{}'",
                account_id.replace('\'', "''")
            )
        })
        .unwrap_or_default();
    let script = format!(
        r#"
$headers = @{{
  Authorization = 'Bearer {}'
  Accept = 'application/json'
  Referer = 'https://chatgpt.com/codex/settings/usage'
  originator = 'codex-desktop'
  'User-Agent' = 'Codex Switch'
}}
{}
$response = Invoke-RestMethod -Uri 'https://chatgpt.com/backend-api/wham/usage' -Method Get -Headers $headers -TimeoutSec 20
function Read-Left($window) {{
  if ($null -eq $window -or $null -eq $window.used_percent) {{ return $null }}
  return [Math]::Max(100 - [double]$window.used_percent, 0)
}}
function Read-Reset($window) {{
  if ($null -eq $window -or $null -eq $window.reset_at) {{ return '-' }}
  return ([DateTimeOffset]::FromUnixTimeSeconds([int64]$window.reset_at).LocalDateTime).ToString('yyyy-MM-dd HH:mm')
}}
$primary = $response.rate_limit.primary_window
$secondary = $response.rate_limit.secondary_window
[pscustomobject]@{{
  plan = [string]$response.plan_type
  primaryLeft = Read-Left $primary
  secondaryLeft = Read-Left $secondary
  reset = Read-Reset $primary
}} | ConvertTo-Json -Compress
"#,
        access_token.replace('\'', "''"),
        account_line
    );
    let raw = run_powershell(&script)?;
    let value: Value = serde_json::from_str(raw.trim())
        .map_err(|error| format!("Failed to parse usage response for {profile_name}: {error}"))?;

    Ok(ProfileUsage {
        profile_name: profile_name.to_string(),
        plan: value_string(&value, "plan").unwrap_or_else(|| "-".to_string()),
        primary_left: value.get("primaryLeft").and_then(Value::as_f64),
        secondary_left: value.get("secondaryLeft").and_then(Value::as_f64),
        reset: value_string(&value, "reset").unwrap_or_else(|| "-".to_string()),
    })
}

fn view_for_profile(meta: &ProfileMeta, active: Option<&String>) -> ProfileView {
    let base = profile_path(&meta.name);
    let auth_path = base.join(AUTH_FILE);
    let config_path = base.join(CONFIG_FILE);
    let session = session_from_auth(&auth_path);
    ProfileView {
        name: meta.name.clone(),
        active: active == Some(&meta.name),
        plan: session.auth_mode.unwrap_or_else(|| "Unknown".to_string()),
        email: session.email.unwrap_or_else(|| "-".to_string()),
        account_id: session.account_id.unwrap_or_else(|| "-".to_string()),
        primary_left: None,
        secondary_left: None,
        reset: session.last_refresh.unwrap_or_else(|| "-".to_string()),
        updated: meta.updated_at.clone(),
        auth_size: file_size_label(&auth_path),
        config_size: file_size_label(&config_path),
        auth_text: read_text(&auth_path),
        config_text: read_text(&config_path),
    }
}

fn app_snapshot(state: Option<State<'_, RuntimeState>>) -> Result<AppSnapshot, String> {
    let db = load_db()?;
    let mut profiles = Vec::new();
    for meta in db.profiles.values() {
        profiles.push(view_for_profile(meta, db.active.as_ref()));
    }
    let launch_target = state.and_then(|runtime| {
        runtime
            .launch_target
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|app| app.target.clone()))
    });
    Ok(AppSnapshot {
        active: db.active,
        codex_dir: codex_dir().display().to_string(),
        data_dir: locate_data_dir().display().to_string(),
        launch_target,
        profiles,
    })
}

fn find_shell_app() -> Option<ShellApp> {
    let script = r#"
$apps = Get-StartApps -ErrorAction SilentlyContinue |
  Where-Object { $_.Name -match '^(ChatGPT|Codex)\b' -or $_.AppID -match 'ChatGPT|Codex' } |
  Select-Object -First 1
if ($apps) { "$($apps.Name)|shell:AppsFolder\$($apps.AppID)" }
"#;
    let raw = run_powershell(script).ok()?;
    let (name, target) = raw.trim().split_once('|')?;
    Some(ShellApp {
        name: name.to_string(),
        target: target.to_string(),
    })
}

fn hard_stop_codex_app() -> Result<String, String> {
    let script = r#"
$taskkill = Join-Path $env:SystemRoot 'System32\taskkill.exe'
& $taskkill /IM ChatGPT.exe /T /F | Out-String | Write-Output
& $taskkill /IM Codex.exe /T /F | Out-String | Write-Output
& $taskkill /IM codex.exe /T /F | Out-String | Write-Output
Start-Sleep -Milliseconds 200
$count = @(Get-Process -Name ChatGPT,Codex,codex -ErrorAction SilentlyContinue).Count
"Hard kill complete. Main process count after: $count."
"#;
    run_powershell(script)
}

fn start_codex_app(runtime: &RuntimeState) -> Result<String, String> {
    let app = {
        let mut guard = runtime
            .launch_target
            .lock()
            .map_err(|_| "launch target cache is locked.".to_string())?;
        if let Some(found) = find_shell_app() {
            *guard = Some(found);
        }
        guard.clone()
    }
    .ok_or_else(|| "No ChatGPT/Codex app was found from Get-StartApps.".to_string())?;

    let mut command = Command::new("explorer.exe");
    command.arg(&app.target);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
        .spawn()
        .map_err(|error| format!("Failed to start explorer.exe for {}: {error}", app.target))?;

    Ok(format!(
        "App started from StartApps/AppsFolder: {} -> {}.",
        app.name, app.target
    ))
}

fn open_path(path: &Path) -> Result<CommandResult, String> {
    let script = format!(
        "Start-Process -LiteralPath '{}'",
        path.display().to_string().replace('\'', "''")
    );
    run_powershell(&script)?;
    Ok(CommandResult::ok(format!("Opened {}.", path.display())))
}

fn current_login_state() -> LoginState {
    let auth_path = current_file(AUTH_FILE);
    let config_path = current_file(CONFIG_FILE);
    let auth_exists = auth_path.exists();
    let config_exists = config_path.exists();

    if !auth_exists && !config_exists {
        return LoginState {
            ready: false,
            signature: None,
            message: "Waiting for Codex to create login files...".to_string(),
        };
    }
    if config_exists && !auth_exists {
        return LoginState {
            ready: false,
            signature: None,
            message: "Codex opened. Waiting for account login...".to_string(),
        };
    }
    if auth_exists && !config_exists {
        return LoginState {
            ready: false,
            signature: None,
            message: "Auth file detected. Waiting for config.toml...".to_string(),
        };
    }

    let auth_len = fs::metadata(&auth_path).map(|m| m.len()).unwrap_or(0);
    let config_len = fs::metadata(&config_path).map(|m| m.len()).unwrap_or(0);
    if auth_len <= MIN_AUTH_BYTES {
        return LoginState {
            ready: false,
            signature: None,
            message: "Login detected. Waiting for auth.json to finish...".to_string(),
        };
    }
    if config_len <= MIN_CONFIG_BYTES {
        return LoginState {
            ready: false,
            signature: None,
            message: "Login detected. Waiting for config.toml to finish...".to_string(),
        };
    }

    LoginState {
        ready: true,
        signature: Some(format!("auth.json={auth_len}|config.toml={config_len}")),
        message: "New login files detected. Waiting for file sizes to stabilize...".to_string(),
    }
}

fn build_tray_menu(
    app: &tauri::AppHandle,
    profiles: &[TrayProfile],
) -> Result<Menu<tauri::Wry>, tauri::Error> {
    let menu = Menu::new(app)?;
    let window_visible = app
        .get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    let toggle_label = if window_visible {
        "Hide window"
    } else {
        "Show window"
    };

    menu.append(&MenuItem::with_id(
        app,
        "tray_toggle_window",
        toggle_label,
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "tray_login",
        "Login New Account",
        true,
        None::<&str>,
    )?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    if profiles.is_empty() {
        menu.append(&MenuItem::with_id(
            app,
            "tray_no_profiles",
            "No profiles",
            false,
            None::<&str>,
        )?)?;
    } else {
        for (index, profile) in profiles.iter().enumerate() {
            let label = if profile.active {
                format!("* {}", profile.name)
            } else {
                profile.name.clone()
            };
            menu.append(&MenuItem::with_id(
                app,
                format!("tray_switch_{index}"),
                label,
                true,
                None::<&str>,
            )?)?;
        }
    }

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "tray_exit",
        "Exit",
        true,
        None::<&str>,
    )?)?;

    Ok(menu)
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    let _ = refresh_tray_menu(app);
}

fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let _ = refresh_tray_menu(app);
}

fn toggle_main_window(app: &tauri::AppHandle) {
    let visible = app
        .get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    if visible {
        hide_main_window(app);
    } else {
        show_main_window(app);
    }
}

fn refresh_tray_menu(app: &tauri::AppHandle) -> Result<(), String> {
    let runtime = app.state::<RuntimeState>();
    let profiles = runtime
        .tray_profiles
        .lock()
        .map_err(|_| "tray profile cache is locked.".to_string())?
        .clone();
    let menu = build_tray_menu(app, &profiles).map_err(|error| error.to_string())?;
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| "tray icon not found".to_string())?;
    tray.set_menu(Some(menu)).map_err(|error| error.to_string())
}

#[tauri::command]
fn start_window_drag(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    window.start_dragging().map_err(|error| error.to_string())?;
    Ok(CommandResult::ok("Window drag started."))
}

#[tauri::command]
fn show_side_panel(
    app: tauri::AppHandle,
    title: String,
    body_html: String,
    _base_width: f64,
) -> Result<CommandResult, String> {
    app.emit("side-panel-content", PanelPayload { title, body_html })
        .map_err(|error| error.to_string())?;
    Ok(CommandResult::ok("Side panel shown."))
}

#[tauri::command]
fn hide_side_panel(_app: tauri::AppHandle, _base_width: f64) -> Result<CommandResult, String> {
    Ok(CommandResult::ok("Side panel hidden."))
}

#[tauri::command]
fn list_profiles(state: State<'_, RuntimeState>) -> Result<AppSnapshot, String> {
    app_snapshot(Some(state))
}

#[tauri::command]
fn switch_profile(
    state: State<'_, RuntimeState>,
    profile_name: String,
) -> Result<AppSnapshot, String> {
    hard_stop_codex_app()?;
    let db = load_db()?;
    let active = db.active.clone();
    let _ = backup_current("before-switch", false)?;
    if let Some(active_name) = active {
        if db.profiles.contains_key(&active_name) && current_files_complete() {
            save_current_to_profile(&active_name)?;
        }
    }
    restore_profile(&profile_name)?;
    thread::sleep(Duration::from_millis(500));
    start_codex_app(&state)?;
    app_snapshot(Some(state))
}

#[tauri::command]
fn login_new_account(state: State<'_, RuntimeState>) -> Result<CommandResult, String> {
    let stop_message = hard_stop_codex_app()?;
    let db = load_db()?;
    let mut saved_message = "No active profile was saved.".to_string();
    if let Some(active_name) = db.active {
        if db.profiles.contains_key(&active_name) && current_files_complete() {
            save_current_to_profile(&active_name)?;
            saved_message = format!("Saved active profile: {active_name}.");
        }
    }
    let backup_message = match backup_current("before-new-login", true)? {
        Some(path) => format!("Moved current config to backup: {}.", path.display()),
        None => "No current auth/config files were available to move.".to_string(),
    };
    thread::sleep(Duration::from_millis(500));
    let start_message = start_codex_app(&state)?;
    Ok(CommandResult::ok(format!(
        "Login flow started. {stop_message} {saved_message} {backup_message} {start_message}"
    )))
}

#[tauri::command]
fn get_login_state() -> LoginState {
    current_login_state()
}

#[tauri::command]
fn refresh_profile_usage(profile_name: String) -> Result<ProfileUsage, String> {
    usage_for_profile(&profile_name)
}

#[tauri::command]
fn finish_login_new_account(state: State<'_, RuntimeState>) -> Result<AppSnapshot, String> {
    let state_now = current_login_state();
    if !state_now.ready {
        return Err(state_now.message);
    }
    let session = session_from_auth(&current_file(AUTH_FILE));
    let base = session
        .name
        .or(session.email)
        .or(session.account_id)
        .unwrap_or_else(|| format!("Codex_Profile_{}", now_stamp()));
    let name = unique_profile_name(&base)?;
    save_current_to_profile(&name)?;
    app_snapshot(Some(state))
}

#[tauri::command]
fn save_current_as_new(
    state: State<'_, RuntimeState>,
    profile_name: Option<String>,
) -> Result<AppSnapshot, String> {
    let name = match profile_name.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        Some(name) => unique_profile_name(&name)?,
        None => {
            let session = session_from_auth(&current_file(AUTH_FILE));
            let base = session
                .name
                .or(session.email)
                .or(session.account_id)
                .unwrap_or_else(|| format!("Codex_Profile_{}", now_stamp()));
            unique_profile_name(&base)?
        }
    };
    save_current_to_profile(&name)?;
    app_snapshot(Some(state))
}

#[tauri::command]
fn delete_profile(
    state: State<'_, RuntimeState>,
    profile_name: String,
) -> Result<AppSnapshot, String> {
    let mut db = load_db()?;
    if !db.profiles.contains_key(&profile_name) {
        return Err("Profile does not exist.".to_string());
    }
    let src = profile_path(&profile_name);
    if src.exists() {
        fs::create_dir_all(trash_dir()).map_err(|error| error.to_string())?;
        let dest = trash_dir().join(format!(
            "{}_{}",
            safe_profile_name(&profile_name),
            now_stamp()
        ));
        fs::rename(&src, &dest)
            .or_else(|_| {
                fs::create_dir_all(&dest)?;
                for entry in fs::read_dir(&src)? {
                    let entry = entry?;
                    fs::rename(entry.path(), dest.join(entry.file_name()))?;
                }
                fs::remove_dir_all(&src)
            })
            .map_err(|error| error.to_string())?;
    }
    db.profiles.remove(&profile_name);
    if db.active.as_ref() == Some(&profile_name) {
        db.active = None;
    }
    save_db(&db)?;
    app_snapshot(Some(state))
}

#[tauri::command]
fn open_chatgpt_codex(state: State<'_, RuntimeState>) -> Result<CommandResult, String> {
    start_codex_app(&state).map(CommandResult::ok)
}

#[tauri::command]
fn open_data_dir() -> Result<CommandResult, String> {
    ensure_dirs()?;
    open_path(&locate_data_dir())
}

#[tauri::command]
fn open_codex_dir() -> Result<CommandResult, String> {
    fs::create_dir_all(codex_dir()).map_err(|error| error.to_string())?;
    open_path(&codex_dir())
}

#[tauri::command]
fn hard_kill_codex() -> Result<CommandResult, String> {
    hard_stop_codex_app().map(CommandResult::ok)
}

#[tauri::command]
fn update_tray_profiles(
    app: tauri::AppHandle,
    profiles: Vec<TrayProfile>,
) -> Result<CommandResult, String> {
    let runtime = app.state::<RuntimeState>();
    *runtime
        .tray_profiles
        .lock()
        .map_err(|_| "tray profile cache is locked.".to_string())? = profiles;
    refresh_tray_menu(&app)?;

    Ok(CommandResult::ok("Tray profiles updated."))
}

#[tauri::command]
fn resize_window(app: tauri::AppHandle, width: f64, height: f64) -> Result<CommandResult, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let target = LogicalSize::new(width, height);
    window
        .set_min_size(None::<LogicalSize<f64>>)
        .map_err(|error| error.to_string())?;
    window
        .set_max_size(None::<LogicalSize<f64>>)
        .map_err(|error| error.to_string())?;
    window.set_size(target).map_err(|error| error.to_string())?;
    window
        .set_min_size(Some(target))
        .map_err(|error| error.to_string())?;
    window
        .set_max_size(Some(target))
        .map_err(|error| error.to_string())?;
    let _ = window.set_resizable(false);

    Ok(CommandResult::ok("Window resized."))
}

#[tauri::command]
fn minimize_window(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    window.minimize().map_err(|error| error.to_string())?;

    Ok(CommandResult::ok("Window minimized."))
}

#[tauri::command]
fn close_window(app: tauri::AppHandle) -> Result<CommandResult, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    window.hide().map_err(|error| error.to_string())?;
    refresh_tray_menu(&app)?;

    Ok(CommandResult::ok("Window hidden to tray."))
}

pub fn run() {
    tauri::Builder::default()
        .manage(RuntimeState {
            launch_target: Mutex::new(find_shell_app()),
            tray_profiles: Mutex::new(Vec::new()),
        })
        .setup(|app| {
            let menu = build_tray_menu(app.handle(), &[])?;
            TrayIconBuilder::with_id(TRAY_ID)
                .tooltip("Codex Switch")
                .icon(APP_ICON)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        toggle_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "tray_toggle_window" {
                toggle_main_window(app);
            } else if id == "tray_login" {
                show_main_window(app);
                let _ = app.emit(
                    "tray-menu-event",
                    TrayMenuPayload {
                        action: "login".to_string(),
                        index: None,
                    },
                );
            } else if id == "tray_exit" {
                app.exit(0);
            } else if let Some(index_text) = id.strip_prefix("tray_switch_") {
                if let Ok(index) = index_text.parse::<usize>() {
                    show_main_window(app);
                    let _ = app.emit(
                        "tray-menu-event",
                        TrayMenuPayload {
                            action: "switch".to_string(),
                            index: Some(index),
                        },
                    );
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            switch_profile,
            login_new_account,
            get_login_state,
            refresh_profile_usage,
            finish_login_new_account,
            save_current_as_new,
            delete_profile,
            open_chatgpt_codex,
            open_data_dir,
            open_codex_dir,
            hard_kill_codex,
            update_tray_profiles,
            start_window_drag,
            show_side_panel,
            hide_side_panel,
            resize_window,
            minimize_window,
            close_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running Codex Switch");
}
