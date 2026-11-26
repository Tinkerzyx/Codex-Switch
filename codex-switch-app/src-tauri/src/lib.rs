use chrono::{DateTime, Duration as ChronoDuration, Utc};
use platform::web_session::IsolatedWebSession;
use platform::AppTarget as ShellApp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    collections::BTreeMap,
    env, fs,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Emitter, LogicalSize, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder,
};
use uuid::Uuid;

mod cloudbase;
mod commands;
mod core;
mod platform;
mod project_sync;
mod services;
mod storage;

const TRAY_ID: &str = "codex-switch-tray";
const FLOATING_BAR_ID: &str = "floating-bar";
const PAYMENT_CAPTURE_ID: &str = "payment-capture";
const ACCOUNT_LOGIN_ID: &str = "account-login";
const STORE_MONTHLY_PAYMENT_URL: &str = "https://catfk.com/item/bpx5pe";
const STORE_WEEKLY_PAYMENT_URL: &str = "https://catfk.com/item/t133lf";
const CATFK_ORDER_URL: &str = "https://catfk.com/shopApi/Pay/order";
const CATFK_MONTHLY_GOODS_KEY: &str = "bpx5pe";
const CATFK_WEEKLY_GOODS_KEY: &str = "t133lf";
const APP_ICON: Image<'_> = tauri::include_image!("icons/icon.png");
const AUTH_FILE: &str = "auth.json";
const CONFIG_FILE: &str = "config.toml";
const INSTANCE_WAKE_ADDR: &str = "127.0.0.1:45421";
const OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const OAUTH_REFRESH_URL: &str = "https://auth.openai.com/oauth/token";
const SUBSCRIPTION_RECHECK_HOURS: i64 = 6;
const TIBO_TIMELINE_URLS: [&str; 4] = [
    "https://syndication.twitter.com/srv/timeline-profile/screen-name/thsottiaux?showReplies=false&transparent=true&lang=en",
    "https://syndication.twitter.com/srv/timeline-profile/screen-name/thsottiaux?showReplies=false&transparent=false&lang=en",
    "https://syndication.twitter.com/srv/timeline-profile/screen-name/thsottiaux?showReplies=false&transparent=true&theme=light",
    "https://syndication.twitter.com/srv/timeline-profile/screen-name/thsottiaux?showReplies=true&transparent=true&lang=en",
];
const TIBO_NITTER_URL: &str = "https://x.noodl3.net/thsottiaux";
static DIAGNOSTIC_LOG_LOCK: Mutex<()> = Mutex::new(());
static APP_SETTINGS_LOCK: Mutex<()> = Mutex::new(());
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

struct RuntimeState {
    launch_target: Mutex<Option<ShellApp>>,
    tray_profiles: Mutex<Vec<TrayProfile>>,
    floating_enabled: Mutex<bool>,
    floating_payload: Mutex<FloatingBarPayload>,
    subscription_refresh_lock: Arc<Mutex<()>>,
    managed_login: Arc<Mutex<ManagedLoginRuntime>>,
}

#[derive(Default)]
struct ManagedLoginRuntime {
    run_id: Option<String>,
    login_id: Option<String>,
    process_id: Option<u32>,
    message: String,
    terminal: bool,
    failed: bool,
    completed: bool,
    web_session_finished: bool,
    web_session_ready: bool,
    subscription_active_until: Option<String>,
    preparing_current: bool,
    rollback_started: bool,
    profile_saved: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginRecoveryRecord {
    run_id: String,
    backup_dir: Option<PathBuf>,
    previous_active: Option<String>,
    #[serde(default)]
    pending_profile_name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SwitchRecoveryRecord {
    from_profile: Option<String>,
    target_profile: String,
    stage: String,
    started_at: String,
    #[serde(default)]
    sync_local_projects: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project_sync: Option<project_sync::ProjectSync>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandResult {
    ok: bool,
    message: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SocialPost {
    text: String,
    url: String,
    created_at: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TiboTimeline {
    latest_post: SocialPost,
    reset_post_at: Option<i64>,
}

fn social_post_timestamp(post: &SocialPost) -> Option<i64> {
    // X snowflake IDs carry UTC creation time, even when a mirror omits dates.
    let id = post.url.rsplit('/').next()?.parse::<u64>().ok()?;
    if id >= (1_u64 << 22) {
        return i64::try_from((id >> 22) + 1_288_834_974_657)
            .ok()
            .map(|ms| ms / 1000);
    }
    let date = post.created_at.as_deref()?;
    DateTime::parse_from_rfc3339(date)
        .or_else(|_| DateTime::parse_from_str(date, "%a %b %d %H:%M:%S %z %Y"))
        .ok()
        .map(|value| value.timestamp())
}

fn summarize_tibo_timeline(posts: Vec<SocialPost>, now: i64) -> Result<TiboTimeline, String> {
    let reset_post_at = posts
        .iter()
        .filter(|post| post.text.to_ascii_lowercase().contains("reset"))
        .filter_map(social_post_timestamp)
        .filter(|timestamp| *timestamp > now - 24 * 60 * 60 && *timestamp <= now)
        .max();
    let latest_post = posts
        .into_iter()
        .max_by_key(|post| {
            post.url
                .rsplit('/')
                .next()
                .and_then(|id| id.parse::<u64>().ok())
                .unwrap_or_default()
        })
        .ok_or_else(|| "No public post was found in Tibo's timeline.".to_string())?;
    Ok(TiboTimeline {
        latest_post,
        reset_post_at,
    })
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
    expires_at: Option<String>,
    reset: String,
    updated: String,
    auth_size: String,
    config_size: String,
    auth_text: String,
    config_text: String,
    source: Option<String>,
    product_id: Option<String>,
    order_id: Option<String>,
    allocation_id: Option<String>,
    entitlement_expires_at: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Db {
    #[serde(default = "storage::schema_version", rename = "schemaVersion")]
    schema_version: u32,
    active: Option<String>,
    profiles: BTreeMap<String, ProfileMeta>,
}

impl Default for Db {
    fn default() -> Self {
        Self {
            schema_version: storage::SCHEMA_VERSION,
            active: None,
            profiles: BTreeMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProfileMeta {
    name: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    product_id: Option<String>,
    #[serde(default)]
    order_id: Option<String>,
    #[serde(default)]
    allocation_id: Option<String>,
    #[serde(default)]
    entitlement_expires_at: Option<String>,
    #[serde(default)]
    subscription_checked_at: Option<String>,
    #[serde(default)]
    subscription_active_until: Option<String>,
    #[serde(default)]
    web_session_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeliverySyncResult {
    delivered_profiles: Vec<String>,
    errors: Vec<String>,
    snapshot: AppSnapshot,
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

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FloatingBarPayload {
    title: String,
    usage: String,
    width: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PaymentStartResult {
    ok: bool,
    message: String,
    trade_no: String,
    total_amount: Option<String>,
    payurl: String,
    qr_image: Option<String>,
    capture_debug: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PaymentOrderResult {
    ok: bool,
    message: String,
    trade_no: String,
    total_amount: Option<String>,
    payurl: String,
    cashier_html: Option<String>,
}

struct StorePaymentProduct {
    goods_key: &'static str,
    item_url: &'static str,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    language: String,
    #[serde(default)]
    sync_local_projects: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            sync_local_projects: false,
        }
    }
}

impl Default for FloatingBarPayload {
    fn default() -> Self {
        Self {
            title: "Codex Switch".to_string(),
            usage: "-".to_string(),
            width: 132.0,
        }
    }
}

#[derive(Default)]
struct SessionInfo {
    name: Option<String>,
    email: Option<String>,
    account_id: Option<String>,
    auth_mode: Option<String>,
    last_refresh: Option<String>,
    access_token: Option<String>,
    subscription_active_until: Option<String>,
}

#[derive(Serialize)]
struct OAuthRefreshRequest<'a> {
    client_id: &'static str,
    grant_type: &'static str,
    refresh_token: &'a str,
}

#[derive(Deserialize)]
struct OAuthRefreshResponse {
    id_token: Option<String>,
    access_token: Option<String>,
    refresh_token: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginState {
    ready: bool,
    signature: Option<String>,
    message: String,
    terminal: bool,
    failed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileUsage {
    profile_name: String,
    plan: String,
    primary_left: Option<f64>,
    primary_reset_at: Option<i64>,
    secondary_left: Option<f64>,
    expires_at: Option<String>,
    reset: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StartFiveHourResult {
    confirmed: bool,
    already_active: bool,
    message: String,
    usage: Option<ProfileUsage>,
    chat_cleanup: String,
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
    chrono::Local::now().format("%Y-%m-%d_%H%M%S").to_string()
}

fn now_text() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn diagnostics_dir() -> PathBuf {
    locate_data_dir().join("diagnostics")
}

fn append_diagnostic_log(message: &str) {
    let Ok(_guard) = DIAGNOSTIC_LOG_LOCK.lock() else {
        return;
    };
    let dir = diagnostics_dir();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let line = format!(
        "{} [v{} pid={}] {}\n",
        Utc::now().to_rfc3339(),
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        message
    );
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("codex-switch.log"))
    {
        let _ = file.write_all(line.as_bytes());
    }
}

fn install_panic_reporter() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let report = format!(
            "Codex Switch v{} crashed at {}\nPID: {}\nPanic: {}\n\nBacktrace:\n{}\n",
            env!("CARGO_PKG_VERSION"),
            Utc::now().to_rfc3339(),
            std::process::id(),
            info,
            std::backtrace::Backtrace::force_capture()
        );
        let dir = diagnostics_dir();
        let _ = fs::create_dir_all(&dir);
        let _ = fs::write(dir.join("latest-crash.txt"), &report);
        let _ = fs::write(
            dir.join(format!("crash-{}.txt", Utc::now().format("%Y%m%d-%H%M%S"))),
            &report,
        );
        append_diagnostic_log("PANIC: see latest-crash.txt");
        default_hook(info);
    }));
}

#[cfg(windows)]
fn configure_native_crash_dumps() {
    let dir = diagnostics_dir();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let key =
        r"HKCU\Software\Microsoft\Windows\Windows Error Reporting\LocalDumps\codex-switch-app.exe";
    let settings = [
        (
            "DumpFolder",
            "REG_EXPAND_SZ",
            dir.to_string_lossy().to_string(),
        ),
        ("DumpType", "REG_DWORD", "1".to_string()),
        ("DumpCount", "REG_DWORD", "5".to_string()),
    ];
    for (name, value_type, value) in settings {
        let mut command = Command::new("reg.exe");
        command.args(["ADD", key, "/v", name, "/t", value_type, "/d", &value, "/f"]);
        command.creation_flags(CREATE_NO_WINDOW);
        match command.output() {
            Ok(output) if output.status.success() => {}
            _ => append_diagnostic_log(&format!("failed to configure native crash dump {name}")),
        }
    }
}

#[cfg(not(windows))]
fn configure_native_crash_dumps() {}

fn codex_dir() -> PathBuf {
    platform::codex_dir()
}

fn locate_data_dir() -> PathBuf {
    platform::data_dir()
}

fn send_wake_to_existing_instance() {
    if let Ok(mut stream) = TcpStream::connect(INSTANCE_WAKE_ADDR) {
        let _ = stream.write_all(b"wake");
    }
}

fn acquire_instance_listener() -> Option<TcpListener> {
    match TcpListener::bind(INSTANCE_WAKE_ADDR) {
        Ok(listener) => Some(listener),
        Err(_) => {
            send_wake_to_existing_instance();
            None
        }
    }
}

fn start_instance_wake_listener(app: tauri::AppHandle, listener: TcpListener) {
    thread::spawn(move || {
        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let mut buffer = [0_u8; 32];
                let message = stream
                    .read(&mut buffer)
                    .ok()
                    .map(|count| String::from_utf8_lossy(&buffer[..count]).trim().to_string())
                    .unwrap_or_default();
                if message == "toggle" {
                    toggle_main_window(&app);
                } else {
                    show_main_window(&app);
                }
            }
        }
    });
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

fn settings_path() -> PathBuf {
    locate_data_dir().join("settings.json")
}

fn login_recovery_path() -> PathBuf {
    locate_data_dir().join("pending-login-recovery.json")
}

fn switch_recovery_path() -> PathBuf {
    locate_data_dir().join("pending-switch-recovery.json")
}

fn move_path_if_needed(src: &Path, dest: &Path) -> Result<(), String> {
    if !src.exists() || dest.exists() {
        return Ok(());
    }

    fs::rename(src, dest)
        .or_else(|_| {
            if src.is_dir() {
                fs::create_dir_all(dest)?;
                for entry in fs::read_dir(src)? {
                    let entry = entry?;
                    fs::rename(entry.path(), dest.join(entry.file_name()))?;
                }
                fs::remove_dir_all(src)
            } else {
                fs::copy(src, dest)
                    .map(|_| ())
                    .and_then(|_| fs::remove_file(src))
            }
        })
        .map_err(|error| {
            format!(
                "Failed to migrate '{}' to '{}': {error}",
                src.display(),
                dest.display()
            )
        })
}

fn migrate_nested_data_dir() -> Result<(), String> {
    let target = locate_data_dir();
    let nested = target.join("codex_switch_data");
    if !nested.join("profiles.json").exists() || target.join("profiles.json").exists() {
        return Ok(());
    }

    fs::create_dir_all(&target).map_err(|error| error.to_string())?;
    for name in ["profiles.json", "profiles", "backups", "trash"] {
        move_path_if_needed(&nested.join(name), &target.join(name))?;
    }
    Ok(())
}

fn ensure_dirs() -> Result<(), String> {
    migrate_nested_data_dir()?;
    storage::private_dir(&locate_data_dir()).map_err(|error| error.to_string())?;
    storage::private_dir(&profiles_dir()).map_err(|error| error.to_string())?;
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
    let db: Db = serde_json::from_str(normalized)
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;
    if db.schema_version > storage::SCHEMA_VERSION {
        return Err("Profile data was created by a newer version of Codex Switch. Update the app before continuing.".into());
    }
    Ok(db)
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

fn normalize_language(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "zh" | "zh-cn" | "zh_cn" | "cn" => "zh-cn".to_string(),
        _ => "en".to_string(),
    }
}

fn load_settings() -> Result<AppSettings, String> {
    ensure_dirs()?;
    let path = settings_path();
    if !path.exists() {
        let settings = AppSettings::default();
        save_settings(&settings)?;
        return Ok(settings);
    }
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let mut settings = serde_json::from_str::<AppSettings>(raw.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;
    settings.language = normalize_language(&settings.language);
    Ok(settings)
}

fn save_settings(settings: &AppSettings) -> Result<(), String> {
    ensure_dirs()?;
    let path = settings_path();
    let tmp = path.with_extension("json.tmp");
    let mut normalized = settings.clone();
    normalized.language = normalize_language(&normalized.language);
    let raw = serde_json::to_string_pretty(&normalized).map_err(|error| error.to_string())?;
    fs::write(&tmp, raw).map_err(|error| error.to_string())?;
    fs::rename(&tmp, &path)
        .or_else(|_| {
            fs::copy(&tmp, &path)
                .map(|_| ())
                .and_then(|_| fs::remove_file(&tmp))
        })
        .map_err(|error| error.to_string())
}

fn save_switch_recovery(record: &SwitchRecoveryRecord) -> Result<(), String> {
    ensure_dirs()?;
    let path = switch_recovery_path();
    let temporary = path.with_extension("json.tmp");
    let raw = serde_json::to_string_pretty(record).map_err(|error| error.to_string())?;
    fs::write(&temporary, raw).map_err(|error| error.to_string())?;
    fs::rename(&temporary, &path)
        .or_else(|_| {
            fs::copy(&temporary, &path)
                .map(|_| ())
                .and_then(|_| fs::remove_file(&temporary))
        })
        .map_err(|error| error.to_string())
}

fn load_switch_recovery() -> Result<Option<SwitchRecoveryRecord>, String> {
    let path = switch_recovery_path();
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    serde_json::from_str(raw.trim_start_matches('\u{feff}'))
        .map(Some)
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))
}

fn set_switch_stage(record: &mut SwitchRecoveryRecord, stage: &str) -> Result<(), String> {
    record.stage = stage.to_string();
    save_switch_recovery(record)?;
    append_diagnostic_log(&format!(
        "profile switch stage={} from={:?} target={}",
        record.stage, record.from_profile, record.target_profile
    ));
    Ok(())
}

fn clear_switch_recovery() -> Result<(), String> {
    let path = switch_recovery_path();
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn current_file(name: &str) -> PathBuf {
    codex_dir().join(name)
}

fn profile_path(name: &str) -> PathBuf {
    let raw = profiles_dir().join(name);
    if raw.exists() {
        return raw;
    }

    let safe = profiles_dir().join(safe_profile_name(name));
    if safe.exists() {
        return safe;
    }

    safe
}

fn managed_files() -> [&'static str; 2] {
    [AUTH_FILE, CONFIG_FILE]
}

fn current_files_complete() -> bool {
    core::auth_files::signature(&codex_dir()).is_ok()
}

fn profile_files_complete(name: &str) -> bool {
    core::auth_files::signature(&profile_path(name)).is_ok()
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

fn save_login_recovery(record: &LoginRecoveryRecord) -> Result<(), String> {
    ensure_dirs()?;
    let path = login_recovery_path();
    let temporary = path.with_extension("json.tmp");
    let raw = serde_json::to_vec_pretty(record).map_err(|error| error.to_string())?;
    fs::write(&temporary, raw).map_err(|error| error.to_string())?;
    fs::rename(&temporary, &path)
        .or_else(|_| {
            fs::copy(&temporary, &path)
                .map(|_| ())
                .and_then(|_| fs::remove_file(&temporary))
        })
        .map_err(|error| format!("Failed to save login recovery state: {error}"))
}

fn restore_pending_login_recovery() -> Result<Option<LoginRecoveryRecord>, String> {
    let path = login_recovery_path();
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let record: LoginRecoveryRecord = serde_json::from_str(raw.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("Failed to parse login recovery state: {error}"))?;

    fs::create_dir_all(codex_dir()).map_err(|error| error.to_string())?;
    for name in managed_files() {
        let current = current_file(name);
        if current.exists() {
            fs::remove_file(&current).map_err(|error| error.to_string())?;
        }
        if let Some(backup_dir) = record.backup_dir.as_deref() {
            let backup = backup_dir.join(name);
            if backup.exists() {
                fs::copy(&backup, &current).map_err(|error| error.to_string())?;
            }
        }
    }

    let mut db = load_db()?;
    if let Some(name) = record.pending_profile_name.as_deref() {
        db.profiles.remove(name);
        let profile_dir = profile_path(name);
        if profile_dir.exists() {
            fs::remove_dir_all(profile_dir).map_err(|error| error.to_string())?;
        }
    }
    db.active = record
        .previous_active
        .clone()
        .filter(|name| db.profiles.contains_key(name));
    save_db(&db)?;
    fs::remove_file(&path).map_err(|error| error.to_string())?;
    Ok(Some(record))
}

fn clear_login_recovery() -> Result<(), String> {
    let path = login_recovery_path();
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn set_pending_login_profile_name(name: &str) -> Result<(), String> {
    let path = login_recovery_path();
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let mut record: LoginRecoveryRecord = serde_json::from_str(raw.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("Failed to parse login recovery state: {error}"))?;
    record.pending_profile_name = Some(name.to_string());
    save_login_recovery(&record)
}

fn save_current_to_profile(profile_name: &str) -> Result<(), String> {
    core::auth_files::signature(&codex_dir())?;

    let dest = profile_path(profile_name);
    // Read once and validate before touching either file in an existing profile.
    let auth_path = current_file(AUTH_FILE);
    let auth_raw = fs::read_to_string(&auth_path).map_err(|error| error.to_string())?;
    let auth: Value = serde_json::from_str(auth_raw.trim_start_matches('\u{feff}'))
        .map_err(|error| error.to_string())?;
    if dest.join(AUTH_FILE).exists() {
        let expected = session_from_auth(&dest.join(AUTH_FILE)).account_id.ok_or(
            "Existing profile has no verifiable account identity; refusing to overwrite it.",
        )?;
        validate_auth_account(&auth, &expected).map_err(|error| {
            append_diagnostic_log(&format!(
                "profile save blocked profile={profile_name}: {error}"
            ));
            error
        })?;
    }
    fs::create_dir_all(&dest).map_err(|error| error.to_string())?;
    replace_json_file(&dest.join(AUTH_FILE), &auth)?;
    if current_file(CONFIG_FILE).exists() {
        fs::copy(current_file(CONFIG_FILE), dest.join(CONFIG_FILE))
            .map_err(|error| error.to_string())?;
    } else if dest.join(CONFIG_FILE).exists() {
        fs::remove_file(dest.join(CONFIG_FILE)).map_err(|error| error.to_string())?;
    }

    let mut db = load_db()?;
    let previous = db.profiles.get(profile_name).cloned();
    let created_at = previous
        .as_ref()
        .map(|item| item.created_at.clone())
        .unwrap_or_else(now_text);
    db.profiles.insert(
        profile_name.to_string(),
        ProfileMeta {
            name: profile_name.to_string(),
            created_at,
            updated_at: now_text(),
            source: previous.as_ref().and_then(|item| item.source.clone()),
            product_id: previous.as_ref().and_then(|item| item.product_id.clone()),
            order_id: previous.as_ref().and_then(|item| item.order_id.clone()),
            allocation_id: previous
                .as_ref()
                .and_then(|item| item.allocation_id.clone()),
            entitlement_expires_at: previous
                .as_ref()
                .and_then(|item| item.entitlement_expires_at.clone()),
            subscription_checked_at: previous
                .as_ref()
                .and_then(|item| item.subscription_checked_at.clone()),
            subscription_active_until: previous
                .as_ref()
                .and_then(|item| item.subscription_active_until.clone()),
            web_session_id: previous
                .as_ref()
                .and_then(|item| item.web_session_id.clone()),
        },
    );
    db.active = Some(profile_name.to_string());
    save_db(&db)
}

fn restore_profile(profile_name: &str) -> Result<(), String> {
    core::auth_files::check_store(&codex_dir())?;
    if !profile_files_complete(profile_name) {
        return Err(
            "Selected profile has invalid credentials or an unsupported credential store."
                .to_string(),
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
        if !profile_path(profile_name).join(name).exists() {
            continue;
        }
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
    session_from_auth_value(&auth)
}

fn session_from_auth_value(auth: &Value) -> SessionInfo {
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
    let subscription_active_until = id_payload
        .get("https://api.openai.com/auth")
        .and_then(|item| item.get("chatgpt_subscription_active_until"))
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
        subscription_active_until,
    }
}

fn validate_auth_account(auth: &Value, expected: &str) -> Result<(), String> {
    let token_account = auth["tokens"]["account_id"].as_str();
    if session_from_auth_value(auth).account_id.as_deref() != Some(expected)
        || token_account.is_some_and(|id| id != expected)
    {
        return Err(
            "Account identity mismatch; refused to overwrite profile credentials.".to_string(),
        );
    }
    Ok(())
}

fn project_renderer_owner(auth_path: &Path) -> Result<String, String> {
    let raw = fs::read_to_string(auth_path).map_err(|error| error.to_string())?;
    let auth: Value = serde_json::from_str(raw.trim_start_matches('\u{feff}'))
        .map_err(|_| "Project sync could not read the Codex account identity.".to_string())?;
    let session = session_from_auth_value(&auth);
    let payload = session
        .access_token
        .as_deref()
        .and_then(decode_jwt_payload)
        .ok_or("Project sync requires an identifiable ChatGPT account.")?;
    let claims = &payload["https://api.openai.com/auth"];
    let user_id = claims["chatgpt_user_id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or("Project sync could not identify the ChatGPT user.")?;
    let account_id = claims["chatgpt_account_id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or("Project sync could not identify the ChatGPT workspace.")?;
    validate_auth_account(&auth, account_id)?;
    Ok(project_sync::renderer_owner(user_id, account_id))
}

fn copy_matching_auth(source: &Path, destination: &Path, expected: &str) -> Result<(), String> {
    // The validated object is exactly the object written, even if source changes meanwhile.
    let raw = fs::read_to_string(source).map_err(|error| error.to_string())?;
    let auth: Value = serde_json::from_str(raw.trim_start_matches('\u{feff}'))
        .map_err(|error| error.to_string())?;
    validate_auth_account(&auth, expected)?;
    let raw = fs::read_to_string(destination).map_err(|error| error.to_string())?;
    let stored: Value = serde_json::from_str(raw.trim_start_matches('\u{feff}'))
        .map_err(|error| error.to_string())?;
    validate_auth_account(&stored, expected)?;
    replace_json_file(destination, &auth)
}

fn parse_subscription_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn subscription_refresh_due(meta: &ProfileMeta, expires_at: Option<&str>) -> bool {
    let now = Utc::now();
    if let Some(expires_at) = expires_at.and_then(parse_subscription_time) {
        if now < expires_at {
            return false;
        }
    } else if meta.web_session_id.is_none() {
        return false;
    }

    meta.subscription_checked_at
        .as_deref()
        .and_then(parse_subscription_time)
        .is_none_or(|checked_at| {
            now - checked_at >= ChronoDuration::hours(SUBSCRIPTION_RECHECK_HOURS)
        })
}

fn web_subscription_probe_script() -> &'static str {
    r#"
window.__cswSubscriptionProbe = "";
(async function () {
  try {
    function collectDateCandidates(value, path, output) {
      if (!value || typeof value !== "object" || output.length >= 40) return;
      Object.keys(value).forEach(function (key) {
        var item = value[key];
        var itemPath = path ? path + "." + key : key;
        if (/(active.?until|current.?period.?end|renewal.?date|next.?billing|expires?.?at|expiration.?date|end.?date)/i.test(key)) {
          if (typeof item === "string" || typeof item === "number") {
            output.push({ path: itemPath, value: String(item) });
          }
        }
        if (item && typeof item === "object") collectDateCandidates(item, itemPath, output);
      });
    }
    function chooseDate(candidates) {
      var priorities = [
        /(^|\.)active_until$/i,
        /(^|\.)current_period_end$/i,
        /(^|\.)renewal_date$/i,
        /(^|\.)next_billing_date$/i,
        /(^|\.)expires_at$/i,
        /(^|\.)expiration_date$/i,
        /(^|\.)end_date$/i
      ];
      for (var p = 0; p < priorities.length; p += 1) {
        for (var i = 0; i < candidates.length; i += 1) {
          if (priorities[p].test(candidates[i].path) && candidates[i].value) return candidates[i].value;
        }
      }
      return "";
    }
    function decodeJwtPayload(token) {
      try {
        var encoded = String(token || "").split(".")[1] || "";
        var normalized = encoded.replace(/-/g, "+").replace(/_/g, "/");
        while (normalized.length % 4) normalized += "=";
        return JSON.parse(atob(normalized));
      } catch (_) {
        return {};
      }
    }
    var sessionResponse = await fetch("https://chatgpt.com/api/auth/session", {
      cache: "no-store",
      credentials: "include",
      headers: { accept: "application/json" }
    });
    if (!sessionResponse.ok) throw new Error("Session request returned HTTP " + sessionResponse.status);
    var session = await sessionResponse.json();
    var accessToken = String(session && session.accessToken || "");
    var accountId = String(session && session.account && session.account.id || "");
    if (!accessToken || !accountId) throw new Error("The web session has no accessToken or accountId");
    if (typeof window.__cswShowLoginTransition === "function") window.__cswShowLoginTransition();
    var jwtPayload = decodeJwtPayload(accessToken);
    var authClaims = jwtPayload && jwtPayload["https://api.openai.com/auth"] || {};
    var urls = [
      "/backend-api/subscriptions?account_id=" + encodeURIComponent(accountId),
      "/backend-api/subscriptions",
      "/backend-api/accounts/" + encodeURIComponent(accountId) + "/subscriptions",
      "/backend-api/accounts/" + encodeURIComponent(accountId) + "/subscription",
      "/backend-api/subscription"
    ];
    var responses = await Promise.all(urls.map(async function (requestUrl) {
      var response = await fetch("https://chatgpt.com" + requestUrl, {
        cache: "no-store",
        credentials: "include",
        headers: {
          accept: "application/json",
          authorization: "Bearer " + accessToken,
          "chatgpt-account-id": accountId
        }
      });
      var body = await response.json().catch(function () { return {}; });
      return { requestUrl: requestUrl, response: response, body: body };
    }));
    var attempts = responses.map(function (result) {
      var body = result.body;
      return {
        path: result.requestUrl.split("?")[0],
        status: result.response.status,
        keys: body && typeof body === "object" ? Object.keys(body).slice(0, 20) : [],
        error: String(body && (body.detail || body.error && (body.error.message || body.error) || body.message) || "").slice(0, 240)
      };
    });
    var selected = responses.find(function (result) { return result.response.ok; }) || responses[0];
    var subscriptionResponse = selected.response;
    var subscription = selected.body;
    if (!subscriptionResponse.ok) {
      window.__cswSubscriptionProbe = JSON.stringify({
        ok: false,
        error: "Subscription request returned HTTP " + subscriptionResponse.status,
        accountId: accountId,
        sessionKeys: Object.keys(session || {}),
        sessionAccountKeys: Object.keys(session && session.account || {}),
        sessionExpires: String(session && session.expires || ""),
        tokenExpiresAt: jwtPayload && jwtPayload.exp || null,
        tokenAccountId: String(authClaims && authClaims.chatgpt_account_id || ""),
        attempts: attempts
      });
      return;
    }
    var dateCandidates = [];
    collectDateCandidates(subscription, "", dateCandidates);
    window.__cswSubscriptionProbe = JSON.stringify({
      ok: true,
      accountId: accountId,
      planType: String(subscription.plan_type || ""),
      activeUntil: chooseDate(dateCandidates),
      dateCandidates: dateCandidates,
      subscriptionKeys: subscription && typeof subscription === "object" ? Object.keys(subscription) : [],
      attempts: attempts
    });
  } catch (error) {
    window.__cswSubscriptionProbe = JSON.stringify({ ok: false, error: String(error) });
  }
})();
"started"
"#
}

fn probe_web_subscription(
    app: &tauri::AppHandle,
    web_session_id: &str,
) -> Result<(Option<String>, Option<String>), String> {
    let label = format!("subscription-probe-{}", Uuid::new_v4().simple());
    let url = tauri::Url::parse("https://chatgpt.com/").map_err(|error| error.to_string())?;
    let session_dir = locate_data_dir().join("web-sessions").join(web_session_id);
    if !session_dir.exists() {
        return Err("The profile's managed web session is missing.".to_string());
    }
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .title("Codex Switch Subscription Probe")
        .inner_size(900.0, 700.0)
        .isolated_session(session_dir)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()
        .map_err(|error| format!("Failed to create the subscription probe: {error}"))?;
    let _ = window.hide();

    thread::sleep(Duration::from_millis(2200));
    window
        .eval(web_subscription_probe_script())
        .map_err(|error| format!("Failed to start the subscription probe: {error}"))?;

    let result = (|| -> Result<(Option<String>, Option<String>), String> {
        for _ in 0..16 {
            thread::sleep(Duration::from_millis(500));
            let (tx, rx) = mpsc::channel();
            window
                .eval_with_callback("window.__cswSubscriptionProbe || \"\"", move |raw| {
                    let _ = tx.send(raw);
                })
                .map_err(|error| format!("Subscription probe callback failed: {error}"))?;
            let raw = rx
                .recv_timeout(Duration::from_millis(5000))
                .map_err(|error| format!("Subscription probe callback timed out: {error}"))?;
            let Some(value) = parse_eval_json(&raw) else {
                continue;
            };
            let diagnostic_path = diagnostics_dir().join(format!(
                "subscription-probe-{}.json",
                safe_profile_name(web_session_id)
            ));
            let _ = fs::create_dir_all(diagnostics_dir());
            let _ = fs::write(
                diagnostic_path,
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string()),
            );
            if value.get("ok").and_then(Value::as_bool) == Some(true) {
                let active_until =
                    value_string(&value, "activeUntil").filter(|text| !text.trim().is_empty());
                let plan_type =
                    value_string(&value, "planType").filter(|text| !text.trim().is_empty());
                append_diagnostic_log(&format!(
                    "subscription probe webSession={} account={} plan={:?} activeUntil={:?} keys={}",
                    web_session_id,
                    value
                        .get("accountId")
                        .and_then(Value::as_str)
                        .unwrap_or("-"),
                    plan_type,
                    active_until,
                    value
                        .get("subscriptionKeys")
                        .and_then(Value::as_array)
                        .map(|items| items.len())
                        .unwrap_or(0)
                ));
                return Ok((active_until, plan_type));
            }
            let error = value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("The subscription probe failed.");
            append_diagnostic_log(&format!(
                "subscription probe failed webSession={web_session_id}: {error}"
            ));
            return Err(error.to_string());
        }
        let error = "The subscription probe did not finish within 10 seconds.".to_string();
        append_diagnostic_log(&format!(
            "subscription probe failed webSession={web_session_id}: {error}"
        ));
        Err(error)
    })();
    let _ = window.close();
    result
}

fn web_usage_probe_script() -> &'static str {
    r#"
window.__cswUsageProbe = "";
(async function () {
  try {
    function readLeft(windowValue) {
      if (!windowValue || typeof windowValue.used_percent !== "number") return null;
      return Math.max(100 - windowValue.used_percent, 0);
    }
    function formatReset(windowValue) {
      if (!windowValue || !windowValue.reset_at) return "-";
      var date = new Date(Number(windowValue.reset_at) * 1000);
      function pad(value) { return String(value).padStart(2, "0"); }
      return pad(date.getMonth() + 1) + "-" + pad(date.getDate()) + " " + pad(date.getHours()) + ":" + pad(date.getMinutes());
    }
    var sessionResponse = await fetch("https://chatgpt.com/api/auth/session", {
      cache: "no-store",
      credentials: "include",
      headers: { accept: "application/json" }
    });
    if (!sessionResponse.ok) throw new Error("Session request returned HTTP " + sessionResponse.status);
    var session = await sessionResponse.json();
    var accessToken = String(session && session.accessToken || "");
    var accountId = String(session && session.account && session.account.id || "");
    if (!accessToken || !accountId) throw new Error("The web session has no accessToken or accountId");
    var usageResponse = await fetch("https://chatgpt.com/backend-api/wham/usage", {
      cache: "no-store",
      credentials: "include",
      headers: {
        accept: "application/json",
        authorization: "Bearer " + accessToken,
        "chatgpt-account-id": accountId,
        originator: "codex-desktop"
      }
    });
    var usage = await usageResponse.json().catch(function () { return {}; });
    if (!usageResponse.ok) {
      var detail = String(usage && (usage.detail || usage.error && (usage.error.message || usage.error) || usage.message) || "");
      throw new Error("Usage request returned HTTP " + usageResponse.status + (detail ? ": " + detail : ""));
    }
    var rateLimit = usage && usage.rate_limit || {};
    var windows = [rateLimit.primary_window, rateLimit.secondary_window].filter(Boolean);
    var fiveHour = windows.find(function (item) { return Number(item.limit_window_seconds) === 18000; }) || rateLimit.primary_window;
    var weekly = windows.find(function (item) { return Number(item.limit_window_seconds) === 604800; });
    if (!weekly && rateLimit.secondary_window && rateLimit.secondary_window !== fiveHour) weekly = rateLimit.secondary_window;
    var fiveHourReset = formatReset(fiveHour);
    var weeklyReset = formatReset(weekly);
    var reset = fiveHourReset !== "-" && weeklyReset !== "-" ? fiveHourReset + " | " + weeklyReset : (fiveHourReset !== "-" ? fiveHourReset : weeklyReset);
    window.__cswUsageProbe = JSON.stringify({
      ok: true,
      accessToken: accessToken,
      accountId: accountId,
      plan: String(usage && usage.plan_type || "-"),
      primaryLeft: readLeft(fiveHour),
      primaryResetAt: fiveHour && Number(fiveHour.reset_at) || null,
      secondaryLeft: readLeft(weekly),
      reset: reset
    });
  } catch (error) {
    window.__cswUsageProbe = JSON.stringify({ ok: false, error: String(error) });
  }
})();
"started"
"#
}

fn update_profile_access_token(
    profile_name: &str,
    access_token: &str,
    account_id: &str,
) -> Result<(), String> {
    let auth_path = profile_path(profile_name).join(AUTH_FILE);
    let raw = fs::read_to_string(&auth_path).map_err(|error| error.to_string())?;
    let mut auth: Value = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    validate_auth_account(&auth, account_id)?;
    if let Some(payload) = decode_jwt_payload(access_token) {
        if payload["https://api.openai.com/auth"]["chatgpt_account_id"]
            .as_str()
            .is_some_and(|id| id != account_id)
        {
            return Err(
                "Web session token belongs to another account; profile unchanged.".to_string(),
            );
        }
    }
    {
        let tokens = auth
            .get_mut("tokens")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "auth.json does not contain a tokens object.".to_string())?;
        tokens.insert(
            "access_token".to_string(),
            Value::String(access_token.to_string()),
        );
        tokens.insert(
            "account_id".to_string(),
            Value::String(account_id.to_string()),
        );
    }
    if let Some(root) = auth.as_object_mut() {
        root.insert(
            "last_refresh".to_string(),
            Value::String(Utc::now().to_rfc3339()),
        );
    }
    replace_json_file(&auth_path, &auth)
}

fn probe_web_usage(
    app: &tauri::AppHandle,
    profile_name: &str,
    web_session_id: &str,
    expires_at: Option<String>,
) -> Result<ProfileUsage, String> {
    let label = format!("usage-probe-{}", Uuid::new_v4().simple());
    let url = tauri::Url::parse("https://chatgpt.com/").map_err(|error| error.to_string())?;
    let session_dir = locate_data_dir().join("web-sessions").join(web_session_id);
    if !session_dir.exists() {
        return Err("The profile's managed web session is missing.".to_string());
    }
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .title("Codex Switch Usage Probe")
        .inner_size(900.0, 700.0)
        .isolated_session(session_dir)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()
        .map_err(|error| format!("Failed to create the usage probe: {error}"))?;
    let _ = window.hide();
    thread::sleep(Duration::from_millis(2200));
    window
        .eval(web_usage_probe_script())
        .map_err(|error| format!("Failed to start the usage probe: {error}"))?;

    let result = (|| -> Result<ProfileUsage, String> {
        for _ in 0..16 {
            thread::sleep(Duration::from_millis(500));
            let (tx, rx) = mpsc::channel();
            window
                .eval_with_callback("window.__cswUsageProbe || \"\"", move |raw| {
                    let _ = tx.send(raw);
                })
                .map_err(|error| format!("Usage probe callback failed: {error}"))?;
            let raw = rx
                .recv_timeout(Duration::from_millis(5000))
                .map_err(|error| format!("Usage probe callback timed out: {error}"))?;
            let Some(value) = parse_eval_json(&raw) else {
                continue;
            };
            if value.get("ok").and_then(Value::as_bool) != Some(true) {
                return Err(value_string(&value, "error")
                    .unwrap_or_else(|| "The web usage probe failed.".to_string()));
            }
            let access_token = value_string(&value, "accessToken")
                .ok_or_else(|| "The web usage probe returned no access token.".to_string())?;
            let account_id = value_string(&value, "accountId")
                .ok_or_else(|| "The web usage probe returned no account ID.".to_string())?;
            update_profile_access_token(profile_name, &access_token, &account_id)?;
            append_diagnostic_log(&format!(
                "usage token refreshed from web session profile={profile_name}"
            ));
            return Ok(ProfileUsage {
                profile_name: profile_name.to_string(),
                plan: value_string(&value, "plan").unwrap_or_else(|| "-".to_string()),
                primary_left: value.get("primaryLeft").and_then(Value::as_f64),
                primary_reset_at: value.get("primaryResetAt").and_then(Value::as_i64),
                secondary_left: value.get("secondaryLeft").and_then(Value::as_f64),
                expires_at,
                reset: value_string(&value, "reset").unwrap_or_else(|| "-".to_string()),
            });
        }
        Err("The web usage probe did not finish within 10 seconds.".to_string())
    })();
    let _ = window.close();
    result
}

fn probe_subscription_in_window_once(
    window: &tauri::WebviewWindow,
) -> Result<Option<(Option<String>, Option<String>)>, String> {
    window
        .eval(web_subscription_probe_script())
        .map_err(|error| format!("Failed to start the web-session probe: {error}"))?;
    thread::sleep(Duration::from_millis(850));
    let (tx, rx) = mpsc::channel();
    window
        .eval_with_callback("window.__cswSubscriptionProbe || \"\"", move |raw| {
            let _ = tx.send(raw);
        })
        .map_err(|error| format!("Web-session probe callback failed: {error}"))?;
    let raw = rx
        .recv_timeout(Duration::from_millis(1600))
        .map_err(|error| format!("Web-session probe callback timed out: {error}"))?;
    let Some(value) = parse_eval_json(&raw) else {
        return Ok(None);
    };
    if value.get("ok").and_then(Value::as_bool) == Some(true) {
        let active_until =
            value_string(&value, "activeUntil").filter(|text| !text.trim().is_empty());
        let plan_type = value_string(&value, "planType").filter(|text| !text.trim().is_empty());
        return Ok(Some((active_until, plan_type)));
    }
    Ok(None)
}

fn login_page_diagnostics(window: &tauri::WebviewWindow) -> Option<String> {
    let (tx, rx) = mpsc::channel();
    window
        .eval_with_callback(
            r#"JSON.stringify({
              url: location.href,
              title: document.title,
              state: document.readyState,
              textLength: ((document.body && document.body.innerText) || "").trim().length,
              htmlLength: ((document.documentElement && document.documentElement.outerHTML) || "").length
            })"#,
            move |raw| {
                let _ = tx.send(raw);
            },
        )
        .ok()?;
    let raw = rx.recv_timeout(Duration::from_millis(1000)).ok()?;
    let value = parse_eval_json(&raw)?;
    Some(format!(
        "url={}; title={}; ready={}; text={}; html={}",
        value.get("url").and_then(Value::as_str).unwrap_or("-"),
        value.get("title").and_then(Value::as_str).unwrap_or("-"),
        value.get("state").and_then(Value::as_str).unwrap_or("-"),
        value.get("textLength").and_then(Value::as_u64).unwrap_or(0),
        value.get("htmlLength").and_then(Value::as_u64).unwrap_or(0),
    ))
}

fn start_web_first_login(
    app: tauri::AppHandle,
    run_id: String,
    managed_login: Arc<Mutex<ManagedLoginRuntime>>,
) {
    thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let window = app
                .get_webview_window(ACCOUNT_LOGIN_ID)
                .ok_or_else(|| "The managed login window is no longer available.".to_string())?;
            thread::sleep(Duration::from_millis(2400));
            let mut blank_checks = 0_u8;
            loop {
                let should_continue = managed_login.lock().ok().is_some_and(|login| {
                    login.run_id.as_deref() == Some(run_id.as_str()) && !login.terminal
                });
                if !should_continue {
                    return Ok(());
                }

                match probe_subscription_in_window_once(&window) {
                    Ok(Some((active_until, _plan_type))) => {
                        if let Ok(mut login) = managed_login.lock() {
                            if login.run_id.as_deref() == Some(run_id.as_str()) {
                                if login.terminal {
                                    return Ok(());
                                }
                                login.subscription_active_until = active_until;
                                login.web_session_ready = true;
                                login.preparing_current = true;
                                login.message = "ChatGPT web session confirmed. Preparing Codex authorization in the same window...".to_string();
                            }
                        }
                        let preparation = prepare_current_for_new_login(&run_id);
                        if let Ok(mut login) = managed_login.lock() {
                            if login.run_id.as_deref() == Some(run_id.as_str()) {
                                login.preparing_current = false;
                            }
                        }
                        let preparation = preparation?;
                        let should_continue = managed_login.lock().ok().is_some_and(|login| {
                            login.run_id.as_deref() == Some(run_id.as_str()) && !login.terminal
                        });
                        if !should_continue {
                            return Ok(());
                        }
                        if let Ok(mut login) = managed_login.lock() {
                            if login.run_id.as_deref() == Some(run_id.as_str()) {
                                login.message = format!(
                                    "ChatGPT web session confirmed. {preparation} Starting Codex authorization..."
                                );
                            }
                        }
                        spawn_managed_codex_login(
                            app.clone(),
                            run_id.clone(),
                            Arc::clone(&managed_login),
                        )?;
                        return Ok(());
                    }
                    Ok(None) => {
                        let diagnostics = login_page_diagnostics(&window);
                        let blank_navigation = diagnostics
                            .as_deref()
                            .is_none_or(|details| details.contains("url=about:blank"));
                        if blank_navigation {
                            blank_checks = blank_checks.saturating_add(1);
                        } else {
                            blank_checks = 0;
                        }
                        if blank_checks >= 2 {
                            let retry_url = tauri::Url::parse(&format!(
                                "https://chatgpt.com/?csw_retry={}",
                                Uuid::new_v4().simple()
                            ))
                            .map_err(|error| error.to_string())?;
                            window.navigate(retry_url).map_err(|error| {
                                format!("Failed to retry the blank ChatGPT page: {error}")
                            })?;
                            blank_checks = 0;
                        }
                        if let Ok(mut login) = managed_login.lock() {
                            if login.run_id.as_deref() == Some(run_id.as_str()) {
                                login.message = match diagnostics {
                                    Some(details) => format!(
                                        "Sign in to ChatGPT in this window to continue. [{details}]"
                                    ),
                                    None => {
                                        "Sign in to ChatGPT in this window to continue.".to_string()
                                    }
                                };
                            }
                        }
                    }
                    Err(error) => {
                        if app.get_webview_window(ACCOUNT_LOGIN_ID).is_none() {
                            return Ok(());
                        }
                        eprintln!("Initial ChatGPT web-session probe will retry: {error}");
                    }
                }
                thread::sleep(Duration::from_millis(900));
            }
        })();

        if let Err(error) = result {
            let _ = cancel_managed_login(
                &app,
                &managed_login,
                &run_id,
                &format!("Login failed: {error}"),
                true,
                true,
            );
        }
    });
}

fn replace_json_file(path: &Path, value: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Invalid auth path: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = parent.join(format!(".auth-{}.tmp", Uuid::new_v4().simple()));
    let raw = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let mut file = storage::private_file(&temporary).map_err(|error| error.to_string())?;
    file.write_all(&raw).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;

    let replace_result = fs::rename(&temporary, path).or_else(|_| {
        fs::copy(&temporary, path)
            .map(|_| ())
            .and_then(|_| fs::remove_file(&temporary))
    });
    if replace_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    replace_result.map_err(|error| format!("Failed to replace {}: {error}", path.display()))
}

fn refresh_profile_subscription(profile_name: &str) -> Result<Option<String>, String> {
    let auth_path = profile_path(profile_name).join(AUTH_FILE);
    let raw = fs::read_to_string(&auth_path).map_err(|error| error.to_string())?;
    let mut auth: Value = serde_json::from_str(raw.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("Failed to parse {}: {error}", auth_path.display()))?;
    let previous_session = session_from_auth(&auth_path);
    let refresh_token = auth
        .get("tokens")
        .and_then(|tokens| tokens.get("refresh_token"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "auth.json does not contain tokens.refresh_token.".to_string())?
        .to_string();

    let response = reqwest::blocking::Client::new()
        .post(OAUTH_REFRESH_URL)
        .json(&OAuthRefreshRequest {
            client_id: OAUTH_CLIENT_ID,
            grant_type: "refresh_token",
            refresh_token: &refresh_token,
        })
        .timeout(Duration::from_secs(25))
        .send()
        .map_err(|error| format!("Subscription token refresh failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "Subscription token refresh returned HTTP {status}."
        ));
    }
    let refreshed: OAuthRefreshResponse = response
        .json()
        .map_err(|error| format!("Subscription token refresh returned invalid JSON: {error}"))?;

    let tokens = auth
        .get_mut("tokens")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "auth.json does not contain a tokens object.".to_string())?;
    if let Some(id_token) = refreshed.id_token.filter(|value| !value.trim().is_empty()) {
        tokens.insert("id_token".to_string(), Value::String(id_token));
    }
    if let Some(access_token) = refreshed
        .access_token
        .filter(|value| !value.trim().is_empty())
    {
        tokens.insert("access_token".to_string(), Value::String(access_token));
    }
    if let Some(refresh_token) = refreshed
        .refresh_token
        .filter(|value| !value.trim().is_empty())
    {
        tokens.insert("refresh_token".to_string(), Value::String(refresh_token));
    }
    if let Some(root) = auth.as_object_mut() {
        root.insert(
            "last_refresh".to_string(),
            Value::String(Utc::now().to_rfc3339()),
        );
    }
    let expected = previous_session
        .account_id
        .as_deref()
        .ok_or("Cannot refresh a profile with no account identity.")?;
    validate_auth_account(&auth, expected)?;
    replace_json_file(&auth_path, &auth)?;

    let db = load_db()?;
    if db.active.as_deref() == Some(profile_name) {
        let current_path = current_file(AUTH_FILE);
        let current_session = session_from_auth(&current_path);
        if previous_session.account_id.is_some()
            && current_session.account_id == previous_session.account_id
        {
            replace_json_file(&current_path, &auth)?;
        }
    }

    Ok(session_from_auth(&auth_path).subscription_active_until)
}

fn refresh_profile_subscription_if_due(app: &tauri::AppHandle, profile_name: &str) {
    let check = (|| -> Result<(), String> {
        let mut db = load_db()?;
        let meta = db
            .profiles
            .get(profile_name)
            .cloned()
            .ok_or_else(|| format!("Profile '{profile_name}' does not exist."))?;
        let auth_path = profile_path(profile_name).join(AUTH_FILE);
        let expires_at = meta
            .subscription_active_until
            .clone()
            .or_else(|| session_from_auth(&auth_path).subscription_active_until);
        if !subscription_refresh_due(&meta, expires_at.as_deref()) {
            return Ok(());
        }

        let refresh_result = if let Some(web_session_id) = meta.web_session_id.as_deref() {
            probe_web_subscription(app, web_session_id).map(|(active_until, _)| active_until)
        } else {
            refresh_profile_subscription(profile_name)
        };
        match &refresh_result {
            Ok(active_until) => {
                if let Some(meta) = db.profiles.get_mut(profile_name) {
                    meta.subscription_checked_at = Some(Utc::now().to_rfc3339());
                    // A successful response with no date must clear a stale expired date.
                    meta.subscription_active_until = active_until.clone();
                }
                save_db(&db)?;
            }
            Err(error) => {
                append_diagnostic_log(&format!(
                    "subscription refresh failed profile={profile_name}: {error}"
                ));
                return Err(error.clone());
            }
        }
        refresh_result.map(|_| ())
    })();

    if let Err(error) = check {
        eprintln!("Subscription refresh skipped for {profile_name}: {error}");
    }
}

fn usage_for_profile(app: &tauri::AppHandle, profile_name: &str) -> Result<ProfileUsage, String> {
    refresh_profile_subscription_if_due(app, profile_name);
    let stored_auth_path = profile_path(profile_name).join(AUTH_FILE);
    let db = load_db().ok();
    let meta = db
        .as_ref()
        .and_then(|value| value.profiles.get(profile_name).cloned());
    let current_auth_path = current_file(AUTH_FILE);
    let stored_session = session_from_auth(&stored_auth_path);
    let use_current_auth = db.as_ref().and_then(|value| value.active.as_deref())
        == Some(profile_name)
        && current_auth_path.exists()
        && stored_session.account_id.is_some()
        && session_from_auth(&current_auth_path).account_id == stored_session.account_id;
    let auth_path = if use_current_auth {
        &current_auth_path
    } else {
        &stored_auth_path
    };
    let session = session_from_auth(&auth_path);
    let expires_at = meta
        .as_ref()
        .and_then(|item| item.subscription_active_until.clone())
        .or_else(|| session.subscription_active_until.clone());
    let access_token = session
        .access_token
        .ok_or_else(|| "auth.json does not contain tokens.access_token.".to_string())?;
    let value = match services::http::usage(&access_token, session.account_id.as_deref()) {
        Ok(value) => value,
        Err(error) => {
            if let Some(web_session_id) = meta.and_then(|item| item.web_session_id) {
                append_diagnostic_log(&format!("usage request failed profile={profile_name}; retrying with web session: {error}"));
                return probe_web_usage(app, profile_name, &web_session_id, expires_at);
            }
            return Err(error);
        }
    };

    if use_current_auth {
        if let Some(expected) = stored_session.account_id.as_deref() {
            if let Err(error) = copy_matching_auth(&current_auth_path, &stored_auth_path, expected)
            {
                append_diagnostic_log(&format!(
                    "usage auth sync blocked profile={profile_name}: {error}"
                ));
            }
        }
    }

    Ok(ProfileUsage {
        profile_name: profile_name.to_string(),
        plan: value_string(&value, "plan").unwrap_or_else(|| "-".to_string()),
        primary_left: value.get("primaryLeft").and_then(Value::as_f64),
        primary_reset_at: value.get("primaryResetAt").and_then(Value::as_i64),
        secondary_left: value.get("secondaryLeft").and_then(Value::as_f64),
        expires_at,
        reset: value_string(&value, "reset").unwrap_or_else(|| "-".to_string()),
    })
}

fn sanitize_five_hour_text(text: &str) -> String {
    let mut bearer = false;
    text.split_whitespace()
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            let credential_key = [
                "access_token",
                "refresh_token",
                "id_token",
                "api_key",
                "authorization",
            ]
            .iter()
            .any(|key| lower.contains(key));
            let secret = bearer
                || credential_key
                || word.contains("sk-")
                || word.split('.').count() == 3 && word.contains("eyJ");
            bearer = word
                .trim_matches(|c: char| !c.is_alphanumeric())
                .eq_ignore_ascii_case("Bearer")
                || credential_key;
            if secret {
                "[redacted]".to_string()
            } else {
                word.chars().take(1024).collect()
            }
        })
        .take(128)
        .collect::<Vec<_>>()
        .join(" ")
}

struct FiveHourTrace {
    id: String,
    profile: String,
}

impl FiveHourTrace {
    fn new(id: &str, profile: &str) -> Result<Self, String> {
        let id = Uuid::parse_str(id)
            .map_err(|_| "Invalid diagnostic request ID.".to_string())?
            .to_string();
        Ok(Self {
            id,
            profile: profile.to_string(),
        })
    }

    fn log(&self, stage: &str, details: Value) {
        let record = serde_json::json!({
            "time": Utc::now().to_rfc3339(), "version": env!("CARGO_PKG_VERSION"),
            "requestId": self.id, "profile": self.profile, "stage": stage, "details": details,
        });
        let Ok(_guard) = DIAGNOSTIC_LOG_LOCK.lock() else {
            return;
        };
        let dir = diagnostics_dir().join("five-hour-requests");
        if fs::create_dir_all(&dir).is_err() {
            return;
        }
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(format!("{}.jsonl", self.id)))
        {
            let _ = writeln!(file, "{record}");
        }
    }

    fn usage(&self, stage: &str, usage: &ProfileUsage) {
        let now = Utc::now().timestamp();
        self.log(stage, serde_json::json!({
            "primaryLeft": usage.primary_left, "primaryResetAt": usage.primary_reset_at,
            "resetTimeUtc": usage.primary_reset_at.and_then(|reset| DateTime::from_timestamp(reset, 0)).map(|time| time.to_rfc3339()),
            "secondsBehindRollingReset": usage.primary_reset_at.map(|reset| now + FIVE_HOUR_WINDOW_SECONDS - reset),
        }));
    }
}

#[tauri::command]
fn log_five_hour_event(
    request_id: String,
    profile_name: String,
    stage: String,
    details: Option<Value>,
) -> Result<(), String> {
    if ![
        "button_clicked",
        "blocked",
        "confirmation_cancelled",
        "confirmation_accepted",
        "invoke_sent",
        "ui_confirmed",
        "ui_unconfirmed",
        "ui_failed",
        "automatic_started",
        "auto_queued",
        "auto_queue_skipped",
        "auto_confirmed",
        "auto_retry_scheduled",
    ]
    .contains(&stage.as_str())
    {
        return Err("Unsupported diagnostic stage.".to_string());
    }
    let mut safe_details = serde_json::Map::new();
    if let Some(Value::Object(details)) = details {
        for (key, value) in details {
            if [
                "resetAt",
                "sampleSeconds",
                "movementSeconds",
                "queueLength",
                "retryAt",
                "attempt",
                "unconfirmed",
            ]
            .contains(&key.as_str())
                && (value.is_number() || value.is_boolean())
            {
                safe_details.insert(key, value);
            } else if key == "reason" {
                if let Some(text) = value.as_str() {
                    safe_details.insert(key, Value::String(sanitize_five_hour_text(text)));
                }
            }
        }
    }
    FiveHourTrace::new(&request_id, &profile_name)?.log(&stage, Value::Object(safe_details));
    Ok(())
}

fn parse_system_cli_proxy(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let server = if raw.contains('=') {
        raw.split(';')
            .find_map(|entry| entry.trim().strip_prefix("https="))
            .or_else(|| {
                raw.split(';')
                    .find_map(|entry| entry.trim().strip_prefix("http="))
            })?
    } else {
        raw
    };
    let url = if server.contains("://") {
        server.to_string()
    } else {
        format!("http://{server}")
    };
    let parsed = reqwest::Url::parse(&url).ok()?;
    if !["http", "https", "socks5", "socks5h"].contains(&parsed.scheme())
        || parsed.host_str().is_none()
    {
        return None;
    }
    Some(url)
}

fn configure_five_hour_proxy(command: &mut Command, trace: &FiveHourTrace) {
    if [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ]
    .iter()
    .any(|key| env::var_os(key).is_some_and(|value| !value.is_empty()))
    {
        trace.log(
            "cli_network",
            serde_json::json!({"proxySource": "environment"}),
        );
        return;
    }
    if let Some(proxy) = platform::system_proxy().and_then(|raw| parse_system_cli_proxy(&raw)) {
        command.env("HTTPS_PROXY", &proxy).env("HTTP_PROXY", &proxy);
        trace.log("cli_network", serde_json::json!({"proxySource": "system"}));
    } else {
        trace.log(
            "cli_network",
            serde_json::json!({"proxySource": "direct", "systemProxyAvailable": false}),
        );
    }
}

#[tauri::command]
fn get_five_hour_progress(request_id: String) -> Result<String, String> {
    let id = Uuid::parse_str(&request_id).map_err(|_| "Invalid request ID.".to_string())?;
    let path = diagnostics_dir()
        .join("five-hour-requests")
        .join(format!("{id}.jsonl"));
    let raw = fs::read_to_string(path).unwrap_or_default();
    Ok(raw
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|record| value_string(&record, "stage"))
        .find(|stage| !stage.starts_with("ui_"))
        .unwrap_or_else(|| "backend_received".to_string()))
}

fn cleanup_five_hour_chat(
    probe_dir: &Path,
    thread_id: &str,
    action: &str,
    trace: &FiveHourTrace,
) -> bool {
    trace.log(
        "chat_cleanup_start",
        serde_json::json!({"action": action, "threadId": thread_id}),
    );
    let stderr_path = probe_dir.join(format!("cleanup-{action}.stderr.txt"));
    let stderr_file = match File::create(&stderr_path) {
        Ok(file) => file,
        Err(error) => {
            trace.log(
                "chat_cleanup_log_failed",
                serde_json::json!({"error": sanitize_five_hour_text(&error.to_string())}),
            );
            return false;
        }
    };
    let mut command = platform::cli_command();
    command.arg(action).arg(thread_id);
    configure_five_hour_proxy(&mut command, trace);
    if action == "delete" {
        command.arg("--force");
    }
    command
        .current_dir(probe_dir)
        .env("CODEX_HOME", probe_dir)
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_file));
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            trace.log("chat_cleanup_spawn_failed", serde_json::json!({"action": action, "error": sanitize_five_hour_text(&error.to_string())}));
            return false;
        }
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                trace.log("chat_cleanup_exit", serde_json::json!({"action": action, "exitCode": status.code(), "success": status.success(), "stderr": sanitize_five_hour_text(&fs::read_to_string(&stderr_path).unwrap_or_default())}));
                return status.success();
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(100));
            }
            _ => {
                trace.log(
                    "chat_cleanup_timeout",
                    serde_json::json!({"action": action}),
                );
                terminate_process_tree(child.id());
                let _ = child.wait();
                return false;
            }
        }
    }
}

fn save_five_hour_probe_auth(
    source: &Path,
    probe: &Path,
    expected_account_id: &str,
    account_active: bool,
) -> Result<bool, String> {
    if session_from_auth(source).account_id.as_deref() != Some(expected_account_id)
        || session_from_auth(probe).account_id.as_deref() != Some(expected_account_id)
    {
        return Err("The selected profile changed while the request was running.".to_string());
    }
    if account_active {
        return Ok(false);
    }
    copy_matching_auth(probe, source, expected_account_id)?;
    Ok(true)
}

fn run_isolated_five_hour_probe(
    profile_name: &str,
    expected_account_id: &str,
    trace: &FiveHourTrace,
) -> Result<String, String> {
    let account_hash = format!("{:x}", Sha256::digest(expected_account_id.as_bytes()));
    let probe_dir = locate_data_dir()
        .join("five-hour-probes")
        .join(&account_hash[..24]);
    fs::create_dir_all(&probe_dir).map_err(|error| error.to_string())?;
    let thread_path = probe_dir.join("thread-id");
    let mut cleanup_succeeded = false;
    let result = (|| -> Result<String, String> {
        let source_auth = profile_path(profile_name).join(AUTH_FILE);
        let db = load_db()?;
        let selected_account_is_active = db.active.as_deref().is_some_and(|active| {
            session_from_auth(&profile_path(active).join(AUTH_FILE))
                .account_id
                .as_deref()
                == Some(expected_account_id)
        });
        let desktop_auth = current_file(AUTH_FILE);
        let use_desktop_auth = selected_account_is_active
            && session_from_auth(&desktop_auth).account_id.as_deref() == Some(expected_account_id);
        let copy_source = if use_desktop_auth {
            &desktop_auth
        } else {
            &source_auth
        };
        let probe_auth = probe_dir.join(AUTH_FILE);
        fs::copy(copy_source, &probe_auth)
            .map_err(|error| format!("Failed to prepare isolated login: {error}"))?;
        let matches =
            session_from_auth(&probe_auth).account_id.as_deref() == Some(expected_account_id);
        trace.log("isolated_auth_prepared", serde_json::json!({"accountMatches": matches, "isolatedHome": probe_dir, "selectedAccountActive": selected_account_is_active, "authSource": if use_desktop_auth { "desktop_read_only_copy" } else { "stored_profile_copy" }}));
        if !matches {
            return Err("The isolated login does not match the selected account.".to_string());
        }
        let previous_thread = if thread_path.exists() {
            let id = fs::read_to_string(&thread_path).map_err(|error| error.to_string())?;
            let id = id.trim().to_string();
            Uuid::parse_str(&id).map_err(|_| "The saved probe chat ID is invalid.".to_string())?;
            Some(id)
        } else {
            None
        };
        let output_path = probe_dir.join("last-run.jsonl");
        let output_file = File::create(&output_path).map_err(|error| error.to_string())?;
        let stderr_path = probe_dir.join("last-run.stderr.txt");
        let stderr_file = File::create(&stderr_path).map_err(|error| error.to_string())?;

        let mut command = platform::cli_command();
        command.arg("exec");
        configure_five_hour_proxy(&mut command, trace);
        if let Some(ref thread_id) = previous_thread {
            command.arg("resume");
            command
                .arg("--json")
                .arg("--ignore-user-config")
                .arg("--ignore-rules")
                .arg("--skip-git-repo-check")
                .arg(thread_id);
        } else {
            command
                .arg("--json")
                .arg("--ignore-user-config")
                .arg("--ignore-rules")
                .arg("--skip-git-repo-check")
                .arg("--sandbox")
                .arg("read-only")
                .arg("--cd")
                .arg(&probe_dir);
        }
        command
            .arg("Reply with only OK. Do not call tools or edit files.")
            .current_dir(&probe_dir)
            .env("CODEX_HOME", &probe_dir)
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY")
            .env_remove("OPENAI_BASE_URL")
            .stdin(Stdio::null())
            .stdout(Stdio::from(output_file))
            .stderr(Stdio::from(stderr_file));
        #[cfg(windows)]
        command.creation_flags(CREATE_NO_WINDOW);
        trace.log("cli_spawn_start", serde_json::json!({"command": "codex.cmd exec", "resume": previous_thread.is_some(), "timeoutSeconds": 120, "apiKeyEnvironmentRemoved": true}));
        let started_at = std::time::Instant::now();
        let mut child = command
            .spawn()
            .map_err(|error| format!("Failed to start Codex CLI: {error}"))?;
        trace.log("cli_spawned", serde_json::json!({"pid": child.id()}));
        let deadline = std::time::Instant::now() + Duration::from_secs(120);
        let mut timed_out = false;
        let exit_status = loop {
            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                terminate_process_tree(child.id());
                timed_out = true;
                break child.wait().map_err(|error| error.to_string())?;
            }
            thread::sleep(Duration::from_millis(200));
        };

        let output = fs::read_to_string(&output_path).map_err(|error| error.to_string())?;
        let stderr = fs::read_to_string(&stderr_path).unwrap_or_default();
        let events: Vec<Value> = output
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        let turn_completed = events.iter().any(|event| event["type"] == "turn.completed");
        let mut counts = BTreeMap::<String, usize>::new();
        for event in &events {
            *counts
                .entry(event["type"].as_str().unwrap_or("unknown").to_string())
                .or_default() += 1;
        }
        let errors: Vec<String> = events
            .iter()
            .filter_map(|event| {
                if event["type"] == "error"
                    || event["type"] == "turn.failed"
                    || event["item"]["type"] == "error"
                {
                    event["message"]
                        .as_str()
                        .or(event["error"]["message"].as_str())
                        .or(event["item"]["message"].as_str())
                        .map(sanitize_five_hour_text)
                } else {
                    None
                }
            })
            .collect();
        trace.log("cli_exit", serde_json::json!({"exitCode": exit_status.code(), "timedOut": timed_out, "elapsedMs": started_at.elapsed().as_millis(), "turnCompleted": turn_completed, "eventCounts": counts, "usage": events.iter().find(|event| event["type"] == "turn.completed").and_then(|event| event.get("usage")), "errors": errors, "stderrTail": sanitize_five_hour_text(&stderr.lines().rev().take(12).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n"))}));
        if timed_out || !exit_status.success() || !turn_completed {
            trace.log("cli_request_failed", serde_json::json!({"reason": if timed_out {"timeout"} else if !exit_status.success() {"nonzero_exit"} else {"no_completed_turn"}}));
        }
        let thread_id = output
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|event| event.get("type").and_then(Value::as_str) == Some("thread.started"))
            .and_then(|event| value_string(&event, "thread_id"))
            .or(previous_thread)
            .ok_or_else(|| {
                format!("Codex CLI did not start a chat (exit code {}, timed out: {}). Check diagnostic {}.", exit_status.code().unwrap_or(-1), timed_out, trace.id)
            })?;
        Uuid::parse_str(&thread_id)
            .map_err(|_| "Codex reported an invalid probe chat ID.".to_string())?;
        fs::write(&thread_path, &thread_id).map_err(|error| error.to_string())?;
        let chat_cleanup = if cleanup_five_hour_chat(&probe_dir, &thread_id, "delete", trace) {
            cleanup_succeeded = true;
            "deleted"
        } else if cleanup_five_hour_chat(&probe_dir, &thread_id, "archive", trace) {
            cleanup_succeeded = true;
            "archived"
        } else {
            "reuse_pending"
        };

        let refreshed_session = session_from_auth(&probe_auth);
        trace.log("isolated_auth_verified", serde_json::json!({"accountMatches": refreshed_session.account_id.as_deref() == Some(expected_account_id), "chatCleanup": chat_cleanup}));
        if refreshed_session.account_id.as_deref() != Some(expected_account_id) {
            return Err("The isolated login no longer matches the selected account.".to_string());
        }
        let db = load_db()?;
        let selected_account_is_active = db.active.as_deref().is_some_and(|active| {
            session_from_auth(&profile_path(active).join(AUTH_FILE))
                .account_id
                .as_deref()
                == Some(expected_account_id)
        });
        if !db.profiles.contains_key(profile_name)
            || session_from_auth(&source_auth).account_id.as_deref() != Some(expected_account_id)
        {
            return Err("The selected profile changed while the request was running.".to_string());
        }
        if !save_five_hour_probe_auth(
            &source_auth,
            &probe_auth,
            expected_account_id,
            selected_account_is_active,
        )? {
            trace.log(
                "active_auth_writeback_skipped",
                serde_json::json!({"desktopAuthUnchanged": true}),
            );
        } else {
            trace.log(
                "profile_auth_saved",
                serde_json::json!({"activeProfileUnchanged": true}),
            );
        }
        if timed_out {
            return Err(format!(
                "Network timeout: the isolated Codex CLI did not complete a reply within 120 seconds. Check the system proxy or network connection. Probe chat cleanup: {chat_cleanup}."
            ));
        }
        if !exit_status.success() {
            return Err(format!(
                "The isolated Codex request failed (exit code {}).",
                exit_status.code().unwrap_or(-1)
            ));
        }
        if !turn_completed {
            return Err("The isolated CLI exited without completing a model turn.".to_string());
        }
        Ok(chat_cleanup.to_string())
    })();
    if cleanup_succeeded {
        let _ = fs::remove_dir_all(&probe_dir);
    }
    trace.log(
        "isolated_probe_finished",
        serde_json::json!({"success": result.is_ok(), "directoryRemoved": !probe_dir.exists()}),
    );
    result
}

const FIVE_HOUR_WINDOW_SECONDS: i64 = 5 * 60 * 60;

fn five_hour_usage_is_active(usage: &ProfileUsage, now: i64) -> bool {
    usage.primary_left.is_some_and(|left| left < 99.999)
        && usage.primary_reset_at.is_some_and(|reset| reset > now)
}

fn five_hour_reset_is_stable(
    first_reset: i64,
    latest_reset: i64,
    elapsed_seconds: u64,
    now: i64,
) -> bool {
    elapsed_seconds >= 10
        && (first_reset - latest_reset).abs() <= 2
        && latest_reset > now
        && now + FIVE_HOUR_WINDOW_SECONDS - latest_reset >= 15
}

fn start_five_hour_window_for_profile(
    app: &tauri::AppHandle,
    profile_name: &str,
    trace: &FiveHourTrace,
) -> Result<StartFiveHourResult, String> {
    let db = load_db()?;
    trace.log("profile_validation", serde_json::json!({"exists": db.profiles.contains_key(profile_name), "filesComplete": profile_files_complete(profile_name), "activeProfile": db.active, "selectedIsActive": db.active.as_deref() == Some(profile_name)}));
    if !db.profiles.contains_key(profile_name) || !profile_files_complete(profile_name) {
        return Err("The selected profile is missing or incomplete.".to_string());
    }
    let account_id = session_from_auth(&profile_path(profile_name).join(AUTH_FILE))
        .account_id
        .ok_or_else(|| "The selected profile has no account ID.".to_string())?;
    trace.log("usage_before_start", serde_json::json!({}));
    let before = match usage_for_profile(app, profile_name) {
        Ok(usage) => {
            trace.usage("usage_before", &usage);
            Some(usage)
        }
        Err(error) => {
            trace.log(
                "usage_before_failed",
                serde_json::json!({"error": sanitize_five_hour_text(&error)}),
            );
            None
        }
    };
    if before
        .as_ref()
        .is_some_and(|usage| five_hour_usage_is_active(usage, Utc::now().timestamp()))
    {
        trace.log(
            "already_active",
            serde_json::json!({"reason": "usage_below_100"}),
        );
        return Ok(StartFiveHourResult {
            confirmed: true,
            already_active: true,
            message: "The five-hour window is already active.".to_string(),
            usage: before,
            chat_cleanup: "none".to_string(),
        });
    }
    // A very short CLI turn can fix reset_at while used_percent still rounds to 0.
    // Confirm a stable timestamp before spending quota on another turn.
    if let Some(first) = before.as_ref() {
        if first
            .primary_reset_at
            .is_some_and(|reset| Utc::now().timestamp() + FIVE_HOUR_WINDOW_SECONDS - reset >= 15)
        {
            let observed_at = std::time::Instant::now();
            trace.log(
                "existing_reset_stability_check",
                serde_json::json!({"waitSeconds": 10}),
            );
            thread::sleep(Duration::from_secs(10));
            match usage_for_profile(app, profile_name) {
                Err(error) => trace.log(
                    "existing_reset_check_failed",
                    serde_json::json!({"error": sanitize_five_hour_text(&error)}),
                ),
                Ok(latest) => {
                    trace.usage("existing_reset_check", &latest);
                    let now = Utc::now().timestamp();
                    if five_hour_usage_is_active(&latest, now)
                        || first
                            .primary_reset_at
                            .zip(latest.primary_reset_at)
                            .is_some_and(|(first_reset, latest_reset)| {
                                five_hour_reset_is_stable(
                                    first_reset,
                                    latest_reset,
                                    observed_at.elapsed().as_secs(),
                                    now,
                                )
                            })
                    {
                        trace.log(
                            "already_active",
                            serde_json::json!({"reason": "stable_reset"}),
                        );
                        return Ok(StartFiveHourResult {
                            confirmed: true,
                            already_active: true,
                            message: "The five-hour window is already active.".to_string(),
                            usage: Some(latest),
                            chat_cleanup: "none".to_string(),
                        });
                    }
                }
            }
        }
    }

    let chat_cleanup = run_isolated_five_hour_probe(profile_name, &account_id, trace)?;
    let mut reset_candidate: Option<(i64, std::time::Instant)> = None;
    let mut last_usage = None;
    for attempt in 0..7 {
        if attempt > 0 {
            thread::sleep(Duration::from_secs(5));
        }
        trace.log(
            "usage_after_start",
            serde_json::json!({"attempt": attempt + 1}),
        );
        match usage_for_profile(app, profile_name) {
            Err(error) => trace.log("usage_after_failed", serde_json::json!({"attempt": attempt + 1, "error": sanitize_five_hour_text(&error)})),
            Ok(usage) => {
            trace.usage("usage_after", &usage);
            let now = Utc::now().timestamp();
            let stable_reset = match (reset_candidate, usage.primary_reset_at) {
                (Some((first_reset, observed_at)), Some(latest_reset)) => {
                    if five_hour_reset_is_stable(
                        first_reset,
                        latest_reset,
                        observed_at.elapsed().as_secs(),
                        now,
                    ) {
                        true
                    } else {
                        if (first_reset - latest_reset).abs() > 2 {
                            reset_candidate = Some((latest_reset, std::time::Instant::now()));
                        }
                        false
                    }
                }
                (None, Some(reset)) => {
                    reset_candidate = Some((reset, std::time::Instant::now()));
                    false
                }
                (_, None) => {
                    reset_candidate = None;
                    false
                }
            };
            trace.log("confirmation_decision", serde_json::json!({"attempt": attempt + 1, "usageActive": five_hour_usage_is_active(&usage, now), "stableReset": stable_reset, "candidateReset": reset_candidate.map(|(reset, _)| reset), "candidateAgeSeconds": reset_candidate.map(|(_, observed)| observed.elapsed().as_secs())}));
            if five_hour_usage_is_active(&usage, now) || stable_reset {
                return Ok(StartFiveHourResult {
                    confirmed: true,
                    already_active: false,
                    message: "The five-hour window is confirmed by the server.".to_string(),
                    usage: Some(usage),
                    chat_cleanup,
                });
            }
            last_usage = Some(usage);
            }
        }
    }
    Ok(StartFiveHourResult {
        confirmed: false,
        already_active: false,
        message: "The request completed, but the five-hour reset time could not be confirmed. Refresh usage later.".to_string(),
        usage: last_usage,
        chat_cleanup,
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
        expires_at: meta
            .subscription_active_until
            .clone()
            .or(session.subscription_active_until),
        reset: session.last_refresh.unwrap_or_else(|| "-".to_string()),
        updated: meta.updated_at.clone(),
        auth_size: file_size_label(&auth_path),
        config_size: file_size_label(&config_path),
        auth_text: read_text(&auth_path),
        config_text: read_text(&config_path),
        source: meta.source.clone(),
        product_id: meta.product_id.clone(),
        order_id: meta.order_id.clone(),
        allocation_id: meta.allocation_id.clone(),
        entitlement_expires_at: meta.entitlement_expires_at.clone(),
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
    platform::find_app()
}

fn hard_stop_codex_app() -> Result<String, String> {
    core::auth_files::check_store(&codex_dir())?;
    platform::stop_app()
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
    .ok_or_else(|| "No ChatGPT/Codex desktop app was found. On macOS, install it in Applications or set CODEX_SWITCH_APP_PATH.".to_string())?;
    platform::start_app(&app)
}

fn open_path(path: &Path) -> Result<CommandResult, String> {
    platform::open_path(path)?;
    Ok(CommandResult::ok(format!("Opened {}.", path.display())))
}

fn open_url(url: &str) -> Result<CommandResult, String> {
    let parsed = tauri::Url::parse(url).map_err(|_| "Invalid URL.".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Only HTTP and HTTPS URLs can be opened.".into());
    }
    platform::open_url(url)?;
    Ok(CommandResult::ok("Opened link."))
}

fn html_unescape_basic(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn is_qr_candidate(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("data:image/")
        || lower.contains("qr")
        || lower.contains("qrcode")
        || lower.contains("code_url")
        || lower.contains("alipay")
        || lower.contains("weixin")
        || lower.contains("wechat")
}

fn normalize_url(candidate: &str, base: &str) -> Option<String> {
    let clean = html_unescape_basic(candidate.trim().trim_matches('\\'));
    if clean.starts_with("data:image/") {
        return Some(clean);
    }
    if clean.starts_with("https://") || clean.starts_with("http://") {
        return Some(clean);
    }
    if clean.starts_with("//") {
        return Some(format!("https:{clean}"));
    }
    if clean.starts_with('/') {
        if let Some((scheme, rest)) = base.split_once("://") {
            let host = rest.split('/').next().unwrap_or_default();
            if !host.is_empty() {
                return Some(format!("{scheme}://{host}{clean}"));
            }
        }
    }
    None
}

fn fetch_image_as_data_uri(image_url: &str, referer: &str) -> Option<String> {
    use base64::Engine;
    if image_url.starts_with("data:image/") {
        return Some(image_url.to_string());
    }
    let url = reqwest::Url::parse(image_url).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let response = services::http::client()
        .ok()?
        .get(url)
        .header("Referer", referer)
        .send()
        .ok()?
        .error_for_status()
        .ok()?;
    let content_type = response
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .split(';')
        .next()?
        .to_string();
    let bytes = response.bytes().ok()?;
    Some(format!(
        "data:{content_type};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn parse_eval_json(raw: &str) -> Option<Value> {
    let first = serde_json::from_str::<Value>(raw).ok()?;
    if let Value::String(text) = first {
        serde_json::from_str::<Value>(&text).ok()
    } else {
        Some(first)
    }
}

fn payment_qr_scan_script() -> &'static str {
    r#"
(function () {
  function box(el) {
    var r = el.getBoundingClientRect();
    return { x: Math.round(r.left), y: Math.round(r.top), w: Math.round(r.width), h: Math.round(r.height), area: Math.round(r.width * r.height) };
  }
  function offsetBox(el, offset) {
    var r = el.getBoundingClientRect();
    var ox = offset && offset.x ? offset.x : 0;
    var oy = offset && offset.y ? offset.y : 0;
    return { x: Math.round(r.left + ox), y: Math.round(r.top + oy), w: Math.round(r.width), h: Math.round(r.height), area: Math.round(r.width * r.height) };
  }
  function visible(el) {
    var r = el.getBoundingClientRect();
    var style = window.getComputedStyle(el);
    return r.width >= 128 && r.height >= 128 && style.display !== "none" && style.visibility !== "hidden" && Number(style.opacity || 1) > 0.05;
  }
  function closeToSquare(el) {
    var r = el.getBoundingClientRect();
    var diff = Math.abs(r.width - r.height);
    return diff <= Math.max(32, Math.min(r.width, r.height) * 0.18);
  }
  function nodeText(el) {
    var parts = [];
    var current = el;
    for (var i = 0; current && i < 4; i += 1, current = current.parentElement) {
      parts.push(current.id || "");
      parts.push(current.className || "");
      parts.push(current.getAttribute && (current.getAttribute("aria-label") || ""));
      parts.push(current.getAttribute && (current.getAttribute("data-testid") || ""));
      parts.push(current.innerText || "");
    }
    parts.push(el.alt || "");
    parts.push(el.title || "");
    parts.push(el.src || "");
    return parts.join(" ").toLowerCase();
  }
  function qrContext(el) {
    return /(qr|qrcode|code_url|scan|cashier|pay|payment|alipay|wechat|weixin|二维码|扫码|付款|支付|收银)/i.test(nodeText(el));
  }
  function canvasLooksLikeQr(canvas) {
    try {
      var size = 31;
      var tmp = document.createElement("canvas");
      tmp.width = size;
      tmp.height = size;
      var ctx = tmp.getContext("2d", { willReadFrequently: true });
      ctx.drawImage(canvas, 0, 0, size, size);
      var data = ctx.getImageData(0, 0, size, size).data;
      var dark = 0;
      var light = 0;
      var transitions = 0;
      var last = null;
      for (var y = 0; y < size; y += 1) {
        last = null;
        for (var x = 0; x < size; x += 1) {
          var index = (y * size + x) * 4;
          var alpha = data[index + 3];
          if (alpha < 20) continue;
          var lum = data[index] * 0.299 + data[index + 1] * 0.587 + data[index + 2] * 0.114;
          var bit = lum < 128 ? 1 : 0;
          if (bit) dark += 1; else light += 1;
          if (last !== null && last !== bit) transitions += 1;
          last = bit;
        }
      }
      var total = dark + light;
      var darkRatio = total ? dark / total : 0;
      return total > 500 && darkRatio > 0.18 && darkRatio < 0.72 && transitions > 120;
    } catch (error) {
      return false;
    }
  }
  function score(el) {
    var text = nodeText(el);
    var r = el.getBoundingClientRect();
    var s = Math.min(r.width, r.height);
    if (text.indexOf("qr") >= 0 || text.indexOf("二维码") >= 0) s += 900;
    if (text.indexOf("qrcode") >= 0 || text.indexOf("扫码") >= 0) s += 700;
    if (/(cashier|pay|payment|alipay|wechat|weixin|付款|支付|收银)/i.test(text)) s += 260;
    if (closeToSquare(el)) s += 260;
    if (r.width > 380 || r.height > 380) s -= 500;
    return s;
  }
  function readCanvas(canvas, source, requireQrShape) {
    try {
      var data = canvas.toDataURL("image/png");
      var info = box(canvas);
      if (data && data.indexOf("data:image/") === 0 && data.length > 1000 && (!requireQrShape || canvasLooksLikeQr(canvas))) {
        return JSON.stringify({ ok: true, source: source || "canvas", image: data, box: info });
      }
    } catch (error) {}
    return "";
  }
  function findQrTarget(doc, offset) {
    var wrappers = Array.prototype.slice.call(doc.querySelectorAll('[data-role="qrPayImg"], .qrcode-img-area'));
    var details = [];
    var best = null;
    for (var i = 0; i < wrappers.length; i += 1) {
      var wrapper = wrappers[i];
      var canvas = wrapper.querySelector('canvas');
      var wrapperBox = offsetBox(wrapper, offset);
      var canvasBox = canvas ? offsetBox(canvas, offset) : null;
      details.push({ wrapper: wrapperBox, canvas: canvasBox, hasCanvas: !!canvas });
      if (!canvas || !closeToSquare(canvas)) continue;
      var localCanvasBox = box(canvas);
      if (localCanvasBox.w < 96 || localCanvasBox.h < 96) continue;
      if (!best || localCanvasBox.area > best.localArea) {
        best = { wrapper: wrapperBox, canvas: canvasBox, localArea: localCanvasBox.area };
      }
    }
    if (best) {
      return { found: true, target: best, details: details };
    }
    var frames = Array.prototype.slice.call(doc.querySelectorAll('iframe,frame'));
    for (var j = 0; j < frames.length; j += 1) {
      var frame = frames[j];
      try {
        var frameDoc = frame.contentDocument || (frame.contentWindow && frame.contentWindow.document);
        if (!frameDoc) continue;
        var frameRect = frame.getBoundingClientRect();
        var nextOffset = {
          x: (offset && offset.x ? offset.x : 0) + frameRect.left,
          y: (offset && offset.y ? offset.y : 0) + frameRect.top
        };
        var result = findQrTarget(frameDoc, nextOffset);
        if (result && result.found) return result;
        if (result && result.details && result.details.length) {
          details = details.concat(result.details);
        }
      } catch (error) {
        details.push({ iframeBlocked: true, message: String(error) });
      }
    }
    return { found: false, details: details };
  }
  var qrTarget = findQrTarget(document, { x: 0, y: 0 });
  if (qrTarget && qrTarget.found) {
    return JSON.stringify({
      ok: false,
      capture: true,
      message: "Found visible Alipay QR canvas. Capture its rendered screen rectangle.",
      url: location.href,
      title: document.title,
      dpr: window.devicePixelRatio || 1,
      wrapper: qrTarget.target.wrapper,
      canvas: qrTarget.target.canvas,
      matches: qrTarget.details
    });
  }
  if (qrTarget && qrTarget.details && qrTarget.details.length) {
    return JSON.stringify({
      ok: false,
      message: "Found QR wrapper candidates, but none had a visible square canvas.",
      url: location.href,
      title: document.title,
      dpr: window.devicePixelRatio || 1,
      matches: qrTarget.details
    });
  }
  var candidates = Array.prototype.slice.call(document.querySelectorAll("canvas"))
    .filter(visible)
    .filter(closeToSquare)
    .filter(qrContext)
    .sort(function (a, b) { return score(b) - score(a); });
  for (var i = 0; i < candidates.length; i += 1) {
    var el = candidates[i];
    try {
      if (el.tagName === "CANVAS") {
        var canvasResult = readCanvas(el, "context-canvas", true);
        if (canvasResult) return canvasResult;
      }
    } catch (error) {}
  }
  return JSON.stringify({
    ok: false,
    message: "Rendered cashier did not expose a readable canvas/img/svg QR node.",
    url: location.href,
    title: document.title,
    dpr: window.devicePixelRatio || 1,
    candidates: candidates.length
  });
})()
"#
}

fn capture_qr_from_rendered_page(app: &tauri::AppHandle, payurl: &str) -> Result<String, String> {
    let url = tauri::Url::parse(payurl)
        .map_err(|error| format!("invalid payment URL for capture: {error}"))?;
    if let Some(existing) = app.get_webview_window(PAYMENT_CAPTURE_ID) {
        let _ = existing.close();
        thread::sleep(Duration::from_millis(180));
    }
    let window = WebviewWindowBuilder::new(app, PAYMENT_CAPTURE_ID, WebviewUrl::External(url))
        .title("Codex Switch Payment Capture")
        .inner_size(760.0, 900.0)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()
        .map_err(|error| format!("failed to create capture WebView window: {error}"))?;
    let _ = window.hide();

    thread::sleep(Duration::from_millis(3200));
    let mut last_error = "capture did not run.".to_string();
    for attempt in 1..=8 {
        let (tx, rx) = mpsc::channel();
        match window.eval_with_callback(payment_qr_scan_script(), move |result| {
            let _ = tx.send(result);
        }) {
            Ok(_) => match rx.recv_timeout(Duration::from_millis(1600)) {
                Ok(raw) => {
                    if let Some(value) = parse_eval_json(&raw) {
                        if value.get("ok").and_then(Value::as_bool).unwrap_or(false) {
                            if let Some(image) = value.get("image").and_then(Value::as_str) {
                                if image.starts_with("data:image/") {
                                    let _ = window.close();
                                    return Ok(image.to_string());
                                }
                                if let Some(data_uri) = fetch_image_as_data_uri(image, payurl) {
                                    let _ = window.close();
                                    return Ok(data_uri);
                                }
                                last_error = format!("attempt {attempt}: image candidate could not be fetched: {image}");
                            } else {
                                last_error = format!(
                                    "attempt {attempt}: ok=true but no image field: {value}"
                                );
                            }
                        } else if value
                            .get("capture")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                        {
                            thread::sleep(Duration::from_millis(260));
                            match platform::capture_payment_rect(&window, &value) {
                            Ok(data_uri) => {
                                let _ = window.close();
                                return Ok(data_uri);
                            }
                            Err(devtools_error) => last_error = format!(
                                "attempt {attempt}: DevTools-only capture failed: {devtools_error}; payload={value}"
                            ),
                        }
                        } else {
                            last_error = format!(
                            "attempt {attempt}: no direct QR image and no DevTools capture target; payload={value}"
                        );
                        }
                    } else {
                        last_error =
                            format!("attempt {attempt}: failed to parse eval result: {raw}");
                    }
                }
                Err(error) => {
                    last_error = format!("attempt {attempt}: eval callback timeout/error: {error}");
                }
            },
            Err(error) => {
                last_error = format!("attempt {attempt}: eval_with_callback failed: {error}");
            }
        }
        thread::sleep(Duration::from_millis(650));
    }
    let _ = window.close();
    Err(last_error)
}

fn collect_json_strings(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) => output.push(text.clone()),
        Value::Array(items) => {
            for item in items {
                collect_json_strings(item, output);
            }
        }
        Value::Object(map) => {
            for (key, value) in map {
                if is_qr_candidate(key) {
                    if let Value::String(text) = value {
                        output.push(text.clone());
                    }
                }
                collect_json_strings(value, output);
            }
        }
        _ => {}
    }
}

fn first_quoted_after(html: &str, marker: &str, start: usize) -> Option<(String, usize)> {
    let marker_pos = html[start..].find(marker)? + start + marker.len();
    let rest = &html[marker_pos..];
    let end = rest.find(marker.chars().last()?)?;
    Some((rest[..end].to_string(), marker_pos + end))
}

fn extract_qr_image(html: &str, base_url: &str) -> Option<String> {
    if let Ok(value) = serde_json::from_str::<Value>(html) {
        let mut strings = Vec::new();
        collect_json_strings(&value, &mut strings);
        for text in strings {
            if is_qr_candidate(&text) {
                if let Some(url) = normalize_url(&text, base_url) {
                    return Some(url);
                }
            }
        }
    }

    for marker in [
        "src=\"",
        "src='",
        "data-src=\"",
        "data-src='",
        "href=\"",
        "href='",
    ] {
        let mut index = 0;
        while let Some((candidate, next_index)) = first_quoted_after(html, marker, index) {
            if is_qr_candidate(&candidate) {
                if let Some(url) = normalize_url(&candidate, base_url) {
                    return Some(url);
                }
            }
            index = next_index.saturating_add(1);
        }
    }

    for marker in ["https://", "http://", "data:image/"] {
        let mut index = 0;
        while let Some(pos) = html[index..].find(marker) {
            let start = index + pos;
            let tail = &html[start..];
            let end = tail
                .find(|ch: char| {
                    ch == '"' || ch == '\'' || ch == '<' || ch == '\\' || ch.is_whitespace()
                })
                .unwrap_or(tail.len());
            let candidate = &tail[..end];
            if is_qr_candidate(candidate) {
                if let Some(url) = normalize_url(candidate, base_url) {
                    return Some(url);
                }
            }
            index = start.saturating_add(end).saturating_add(1);
        }
    }

    None
}

fn current_login_state() -> LoginState {
    match core::auth_files::signature(&codex_dir()) {
        Ok(signature) => LoginState {
            ready: true,
            signature: Some(signature),
            message: "Valid login detected. Waiting for credentials to stabilize...".into(),
            terminal: false,
            failed: false,
        },
        Err(message) => LoginState {
            ready: false,
            signature: None,
            message,
            terminal: false,
            failed: false,
        },
    }
}

fn terminate_process_tree(process_id: u32) {
    platform::terminate_process(process_id);
}

fn set_managed_login_status(
    state: &Arc<Mutex<ManagedLoginRuntime>>,
    run_id: &str,
    message: impl Into<String>,
    terminal: bool,
    failed: bool,
    completed: bool,
) {
    if let Ok(mut login) = state.lock() {
        if login.run_id.as_deref() == Some(run_id) {
            login.message = message.into();
            login.terminal = terminal;
            login.failed = failed;
            login.completed = completed;
        }
    }
}

fn write_app_server_message(stdin: &mut impl Write, value: Value) -> Result<(), String> {
    let mut line = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    line.push(b'\n');
    stdin.write_all(&line).map_err(|error| error.to_string())?;
    stdin.flush().map_err(|error| error.to_string())
}

fn open_managed_login_window(
    app: &tauri::AppHandle,
    auth_url: &str,
    run_id: &str,
    managed_login: Arc<Mutex<ManagedLoginRuntime>>,
) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window(ACCOUNT_LOGIN_ID) {
        let url = tauri::Url::parse(auth_url)
            .map_err(|error| format!("Codex returned an invalid login URL: {error}"))?;
        existing
            .navigate(url)
            .map_err(|error| format!("Failed to navigate the managed login window: {error}"))?;
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }
    let url = tauri::Url::parse(auth_url)
        .map_err(|error| format!("Codex returned an invalid login URL: {error}"))?;
    let session_dir = locate_data_dir().join("web-sessions").join(run_id);
    fs::create_dir_all(&session_dir).map_err(|error| error.to_string())?;
    let page_run_id = run_id.to_string();
    let page_login = Arc::clone(&managed_login);
    let window = WebviewWindowBuilder::new(app, ACCOUNT_LOGIN_ID, WebviewUrl::External(url))
        .initialization_script(include_str!("login-transition.js"))
        .title("Sign in for Codex Switch")
        .inner_size(1050.0, 780.0)
        .min_inner_size(760.0, 620.0)
        .isolated_session(session_dir)
        .decorations(true)
        .shadow(true)
        .skip_taskbar(false)
        .focused(true)
        .visible(true)
        .center()
        .on_new_window(|url, _features| {
            let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
            if host == "chatgpt.com"
                || host.ends_with(".chatgpt.com")
                || host == "openai.com"
                || host.ends_with(".openai.com")
            {
                tauri::webview::NewWindowResponse::Allow
            } else {
                tauri::webview::NewWindowResponse::Deny
            }
        })
        .on_page_load(move |_window, payload| {
            if let Ok(mut login) = page_login.lock() {
                if login.run_id.as_deref() == Some(page_run_id.as_str()) && !login.completed {
                    login.message = format!("Login page loaded: {}", payload.url());
                }
            }
        })
        .build()
        .map_err(|error| format!("Failed to create the managed login window: {error}"))?;

    let close_run_id = run_id.to_string();
    let close_app = app.clone();
    let close_login = Arc::clone(&managed_login);
    window.on_window_event(move |event| {
        if !matches!(event, tauri::WindowEvent::Destroyed) {
            return;
        }
        let should_cancel = close_login.lock().ok().is_some_and(|login| {
            login.run_id.as_deref() == Some(close_run_id.as_str())
                && !login.web_session_finished
                && !login.completed
                && !login.rollback_started
        });
        if should_cancel {
            let app = close_app.clone();
            let login = Arc::clone(&close_login);
            let run_id = close_run_id.clone();
            thread::spawn(move || {
                let _ = cancel_managed_login(
                    &app,
                    &login,
                    &run_id,
                    "Login window closed by the user.",
                    false,
                    false,
                );
            });
        }
    });
    Ok(())
}

fn prepare_current_for_new_login(run_id: &str) -> Result<String, String> {
    let stop_message = hard_stop_codex_app()?;
    let db = match load_db() {
        Ok(db) => db,
        Err(error) => return Err(error),
    };
    let previous_active = db.active.clone();
    let mut saved_message = "No active profile was saved.".to_string();
    if let Some(active_name) = previous_active.as_deref() {
        if db.profiles.contains_key(active_name) && current_files_complete() {
            save_current_to_profile(active_name)?;
            saved_message = format!("Saved active profile: {active_name}.");
        }
    }
    let backup_dir = backup_current("before-new-login", false)?;
    let backup_message = match backup_dir.as_deref() {
        Some(path) => format!("Backed up current config to {}.", path.display()),
        None => "No current auth/config files were available to back up.".to_string(),
    };
    save_login_recovery(&LoginRecoveryRecord {
        run_id: run_id.to_string(),
        backup_dir,
        previous_active,
        pending_profile_name: None,
    })?;
    let auth_path = current_file(AUTH_FILE);
    if auth_path.exists() {
        fs::remove_file(&auth_path).map_err(|error| {
            format!("Failed to clear the current auth file before login: {error}")
        })?;
    }
    Ok(format!("{stop_message} {saved_message} {backup_message}"))
}

fn remove_login_web_session_later(run_id: String) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(600));
        let _ = fs::remove_dir_all(locate_data_dir().join("web-sessions").join(run_id));
    });
}

fn cancel_managed_login(
    app: &tauri::AppHandle,
    managed_login: &Arc<Mutex<ManagedLoginRuntime>>,
    run_id: &str,
    reason: &str,
    failed: bool,
    close_window: bool,
) -> Result<String, String> {
    let (process_id, restart_needed) = {
        let mut login = managed_login
            .lock()
            .map_err(|_| "Managed login state is unavailable.".to_string())?;
        if login.run_id.as_deref() != Some(run_id) || login.profile_saved {
            return Ok(login.message.clone());
        }
        if login.rollback_started {
            return Ok(login.message.clone());
        }
        login.rollback_started = true;
        login.terminal = true;
        login.failed = failed;
        login.message = format!("{reason} Restoring the previous profile...");
        (login.process_id.take(), login.web_session_ready)
    };

    if let Some(process_id) = process_id {
        terminate_process_tree(process_id);
    }
    if close_window {
        if let Some(window) = app.get_webview_window(ACCOUNT_LOGIN_ID) {
            let _ = window.close();
        }
    }

    for _ in 0..50 {
        let preparing = managed_login
            .lock()
            .ok()
            .is_some_and(|login| login.preparing_current);
        if !preparing {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }

    let restored = match restore_pending_login_recovery() {
        Ok(restored) => restored,
        Err(error) => {
            let message = format!(
                "{reason} Automatic restoration failed: {error}. Recovery data was kept for the next app launch."
            );
            if let Ok(mut login) = managed_login.lock() {
                if login.run_id.as_deref() == Some(run_id) {
                    login.message = message.clone();
                    login.failed = true;
                    login.web_session_finished = true;
                }
            }
            return Err(message);
        }
    };
    let final_message = if restored.is_some() || restart_needed {
        let runtime = app.state::<RuntimeState>();
        match start_codex_app(&runtime) {
            Ok(_) if restored.is_some() => {
                format!("{reason} The previous profile was restored and Codex was restarted.")
            }
            Ok(_) => format!("{reason} Codex was restarted; no new profile was saved."),
            Err(error) if restored.is_some() => format!(
                "{reason} The previous profile was restored, but Codex could not restart: {error}"
            ),
            Err(error) => format!("{reason} Codex could not restart: {error}"),
        }
    } else {
        format!("{reason} No profile was changed or saved.")
    };
    if let Ok(mut login) = managed_login.lock() {
        if login.run_id.as_deref() == Some(run_id) {
            login.message = final_message.clone();
            login.web_session_finished = true;
        }
    }
    remove_login_web_session_later(run_id.to_string());
    Ok(final_message)
}

fn spawn_managed_codex_login(
    app: tauri::AppHandle,
    run_id: String,
    managed_login: Arc<Mutex<ManagedLoginRuntime>>,
) -> Result<(), String> {
    let mut command = platform::cli_command();
    command.args(["app-server", "--stdio"]);
    command.env("CODEX_HOME", codex_dir());
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to start Codex app-server: {error}"))?;
    let process_id = child.id();
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Codex app-server stdin is unavailable.".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Codex app-server stdout is unavailable.".to_string())?;

    {
        let mut login = managed_login
            .lock()
            .map_err(|_| "Managed login state is unavailable.".to_string())?;
        login.process_id = Some(process_id);
        login.message = "Starting the secure Codex login session...".to_string();
    }

    write_app_server_message(
        &mut stdin,
        serde_json::json!({
            "method": "initialize",
            "id": 1,
            "params": {
                "clientInfo": {
                    "name": "codex-switch",
                    "title": "Codex Switch",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": { "experimentalApi": true }
            }
        }),
    )?;

    thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let reader = BufReader::new(stdout);
            let mut login_started = false;
            for line in reader.lines() {
                let line = line.map_err(|error| error.to_string())?;
                let value: Value = match serde_json::from_str(line.trim()) {
                    Ok(value) => value,
                    Err(_) => continue,
                };

                if value.get("id").and_then(Value::as_i64) == Some(1) && !login_started {
                    write_app_server_message(
                        &mut stdin,
                        serde_json::json!({"method": "initialized"}),
                    )?;
                    write_app_server_message(
                        &mut stdin,
                        serde_json::json!({
                            "method": "account/login/start",
                            "id": 2,
                            "params": {"type": "chatgpt"}
                        }),
                    )?;
                    login_started = true;
                    set_managed_login_status(
                        &managed_login,
                        &run_id,
                        "Preparing the ChatGPT sign-in page...",
                        false,
                        false,
                        false,
                    );
                    continue;
                }

                if value.get("id").and_then(Value::as_i64) == Some(2) {
                    if let Some(error) = value.get("error") {
                        return Err(format!("Codex login could not start: {error}"));
                    }
                    let result = value.get("result").unwrap_or(&Value::Null);
                    let auth_url = result
                        .get("authUrl")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "Codex did not return an OAuth URL.".to_string())?;
                    let login_id = result
                        .get("loginId")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if let Ok(mut login) = managed_login.lock() {
                        if login.run_id.as_deref() == Some(run_id.as_str()) {
                            login.login_id = Some(login_id);
                            login.message =
                                "Complete sign-in in the Codex Switch login window.".to_string();
                        }
                    }
                    open_managed_login_window(&app, auth_url, &run_id, Arc::clone(&managed_login))?;
                    continue;
                }

                if value.get("method").and_then(Value::as_str) == Some("account/login/completed") {
                    let params = value.get("params").unwrap_or(&Value::Null);
                    if params.get("success").and_then(Value::as_bool) == Some(true) {
                        if let Ok(mut login) = managed_login.lock() {
                            if login.run_id.as_deref() == Some(run_id.as_str()) {
                                login.message =
                                    "Codex authorization completed. Saving the new profile..."
                                        .to_string();
                                login.completed = true;
                                login.web_session_finished = true;
                            }
                        }
                        if let Some(window) = app.get_webview_window(ACCOUNT_LOGIN_ID) {
                            let _ = window.close();
                        }
                        let runtime = app.state::<RuntimeState>();
                        let _ = start_codex_app(&runtime);
                        return Ok(());
                    }
                    let error = params
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("Codex rejected the login.");
                    return Err(error.to_string());
                }
            }
            Err("Codex login service stopped before login completed.".to_string())
        })();

        if let Err(error) = result {
            let _ = cancel_managed_login(
                &app,
                &managed_login,
                &run_id,
                &format!("Codex authorization failed: {error}"),
                true,
                true,
            );
        }
        if let Ok(mut login) = managed_login.lock() {
            if login.run_id.as_deref() == Some(run_id.as_str()) {
                login.process_id = None;
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    });
    Ok(())
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

fn ensure_window_on_available_monitor(window: &tauri::WebviewWindow) {
    let Ok(monitors) = window.available_monitors() else {
        return;
    };
    if monitors.is_empty() {
        return;
    }
    let Ok(position) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let left = i64::from(position.x);
    let top = i64::from(position.y);
    let right = left + i64::from(size.width);
    let bottom = top + i64::from(size.height);
    let is_visible = monitors.iter().any(|monitor| {
        let monitor_position = monitor.position();
        let monitor_size = monitor.size();
        let monitor_left = i64::from(monitor_position.x);
        let monitor_top = i64::from(monitor_position.y);
        let monitor_right = monitor_left + i64::from(monitor_size.width);
        let monitor_bottom = monitor_top + i64::from(monitor_size.height);
        let visible_width = right.min(monitor_right) - left.max(monitor_left);
        let visible_height = bottom.min(monitor_bottom) - top.max(monitor_top);
        visible_width >= 80 && visible_height >= 48
    });
    if is_visible {
        return;
    }

    let cursor = window.cursor_position().ok();
    let target = cursor
        .and_then(|cursor_position| {
            monitors.iter().find(|monitor| {
                let monitor_position = monitor.position();
                let monitor_size = monitor.size();
                cursor_position.x >= f64::from(monitor_position.x)
                    && cursor_position.x
                        < f64::from(monitor_position.x) + f64::from(monitor_size.width)
                    && cursor_position.y >= f64::from(monitor_position.y)
                    && cursor_position.y
                        < f64::from(monitor_position.y) + f64::from(monitor_size.height)
            })
        })
        .or_else(|| {
            window.primary_monitor().ok().flatten().and_then(|primary| {
                monitors.iter().find(|monitor| {
                    monitor.position() == primary.position() && monitor.size() == primary.size()
                })
            })
        })
        .unwrap_or(&monitors[0]);
    let target_position = target.position();
    let target_size = target.size();
    let available_width = i64::from(target_size.width);
    let available_height = i64::from(target_size.height);
    let window_width = i64::from(size.width).min(available_width);
    let window_height = i64::from(size.height).min(available_height);
    let x = i64::from(target_position.x) + (available_width - window_width) / 2;
    let y = i64::from(target_position.y) + (available_height - window_height) / 2;
    let _ = window.set_position(PhysicalPosition::new(x as i32, y as i32));
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        ensure_window_on_available_monitor(&window);

        // A short topmost pulse is more reliable than focus alone after sleep
        // or a monitor topology change. Restore normal z-order immediately.
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        let delayed_window = window.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(180));
            ensure_window_on_available_monitor(&delayed_window);
            let _ = delayed_window.set_focus();
            let _ = delayed_window.set_always_on_top(false);
            let _ = delayed_window.set_focus();
        });
    }
    let _ = refresh_tray_menu(app);
}

fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    show_floating_bar_if_enabled(app);
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

fn recover_floating_bar(app: &tauri::AppHandle) {
    if !floating_bar_enabled(app) {
        return;
    }
    let payload = floating_bar_payload(app);
    platform::floating::stop(app);
    let _ = platform::floating::start(app, &payload);
}

fn start_floating_bar_watchdog(app: tauri::AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(5));
        if floating_bar_enabled(&app) {
            let payload = floating_bar_payload(&app);
            let _ = platform::floating::start(&app, &payload);
        }
    });
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

fn floating_bar_enabled(app: &tauri::AppHandle) -> bool {
    app.state::<RuntimeState>()
        .floating_enabled
        .lock()
        .map(|guard| *guard)
        .unwrap_or(false)
}

fn floating_bar_payload(app: &tauri::AppHandle) -> FloatingBarPayload {
    app.state::<RuntimeState>()
        .floating_payload
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

fn hide_floating_bar_window(app: &tauri::AppHandle) {
    platform::floating::stop(app);
    if let Some(window) = app.get_webview_window(FLOATING_BAR_ID) {
        let _ = window.hide();
    }
}

fn show_floating_bar_if_enabled(app: &tauri::AppHandle) {
    if !floating_bar_enabled(app) {
        hide_floating_bar_window(app);
        return;
    }
    let payload = floating_bar_payload(app);
    let _ = platform::floating::start(app, &payload);
}

fn collect_social_posts(value: &Value, posts: &mut Vec<SocialPost>) {
    if let Value::Object(object) = value {
        let text = object
            .get("full_text")
            .or_else(|| object.get("fullText"))
            .and_then(Value::as_str)
            .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|text| !text.is_empty());
        let id = object
            .get("id_str")
            .or_else(|| object.get("rest_id"))
            .or_else(|| object.get("id"))
            .and_then(Value::as_str);
        if let (Some(text), Some(id)) = (text, id) {
            posts.push(SocialPost {
                text,
                url: format!("https://x.com/thsottiaux/status/{id}"),
                created_at: object
                    .get("created_at")
                    .or_else(|| object.get("createdAt"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
        for child in object.values() {
            collect_social_posts(child, posts);
        }
    } else if let Value::Array(items) = value {
        for item in items {
            collect_social_posts(item, posts);
        }
    }
}

fn parse_tibo_posts(html: &str) -> Result<Vec<SocialPost>, String> {
    let marker = html
        .find("id=\"__NEXT_DATA__\"")
        .ok_or_else(|| "X timeline data marker was not found.".to_string())?;
    let json_start = marker
        + html[marker..]
            .find('>')
            .ok_or_else(|| "X timeline data opening tag is incomplete.".to_string())?
        + 1;
    let json_end = json_start
        + html[json_start..]
            .find("</script>")
            .ok_or_else(|| "X timeline data closing tag was not found.".to_string())?;
    let data: Value = serde_json::from_str(&html[json_start..json_end])
        .map_err(|error| format!("X timeline data is invalid: {error}"))?;
    let timeline = data
        .pointer("/props/pageProps/timeline/entries")
        .unwrap_or(&data);
    let mut posts = Vec::new();
    collect_social_posts(timeline, &mut posts);
    if posts.is_empty() {
        return Err("No public post was found in Tibo's X timeline.".to_string());
    }
    Ok(posts)
}

#[cfg(test)]
fn parse_latest_tibo_post(html: &str) -> Result<SocialPost, String> {
    summarize_tibo_timeline(parse_tibo_posts(html)?, Utc::now().timestamp())
        .map(|timeline| timeline.latest_post)
}

fn html_to_plain_text(html: &str) -> String {
    let mut plain = String::with_capacity(html.len());
    let mut in_tag = false;
    for character in html.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => plain.push(character),
            _ => {}
        }
    }
    plain
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_tibo_nitter_posts(html: &str) -> Result<Vec<SocialPost>, String> {
    parse_tibo_nitter_posts_impl(html, false)
}

fn parse_tibo_nitter_posts_impl(html: &str, skip_pinned: bool) -> Result<Vec<SocialPost>, String> {
    let item_marker = "timeline-item";
    let mut cursor = 0;
    let mut posts = Vec::new();
    while let Some(relative_start) = html[cursor..].find(item_marker) {
        let item_start = cursor + relative_start;
        let item_end = html[item_start + item_marker.len()..]
            .find(item_marker)
            .map(|offset| item_start + item_marker.len() + offset)
            .unwrap_or(html.len());
        let item = &html[item_start..item_end];
        cursor = item_end;
        if skip_pinned
            && item
                .split('>')
                .next()
                .is_some_and(|header| header.contains("pinned"))
        {
            continue;
        }
        if !item.contains("data-username=\"thsottiaux\"") {
            continue;
        }
        let status_marker = "href=\"/thsottiaux/status/";
        let Some(status_start) = item.find(status_marker) else {
            continue;
        };
        let id_start = status_start + status_marker.len();
        let id_end = item[id_start..]
            .find(|character: char| !character.is_ascii_digit())
            .map(|offset| id_start + offset)
            .unwrap_or(item.len());
        let id = &item[id_start..id_end];
        if id.is_empty() {
            continue;
        }
        let content_marker = "class=\"tweet-content";
        let Some(content_class) = item.find(content_marker) else {
            continue;
        };
        let Some(content_open_end) = item[content_class..].find('>') else {
            continue;
        };
        let content_start = content_class + content_open_end + 1;
        let Some(content_close) = item[content_start..].find("</div>") else {
            continue;
        };
        let text = html_to_plain_text(&item[content_start..content_start + content_close]);
        if text.is_empty() {
            continue;
        }
        let created_at = item
            .find("class=\"tweet-date\"")
            .and_then(|date_start| {
                item[date_start..]
                    .find("title=\"")
                    .map(|offset| date_start + offset + 7)
            })
            .and_then(|value_start| {
                item[value_start..]
                    .find('"')
                    .map(|offset| html_to_plain_text(&item[value_start..value_start + offset]))
            });
        posts.push(SocialPost {
            text,
            url: format!("https://x.com/thsottiaux/status/{id}"),
            created_at,
        });
    }
    if posts.is_empty() {
        return Err("No current post was found in Tibo's public timeline mirror.".to_string());
    }
    Ok(posts)
}

#[cfg(test)]
fn parse_latest_tibo_nitter(html: &str) -> Result<SocialPost, String> {
    summarize_tibo_timeline(parse_tibo_nitter_posts(html)?, Utc::now().timestamp())
        .map(|timeline| timeline.latest_post)
}

fn nitter_next_page(html: &str) -> Option<tauri::Url> {
    let marker = html.rfind("class=\"show-more")?;
    let tail = &html[marker..];
    let start = tail.find("href=\"")? + 6;
    let end = start + tail[start..].find('"')?;
    let href = tail[start..end].replace("&amp;", "&");
    let base = tauri::Url::parse(TIBO_NITTER_URL).ok()?;
    let url = base.join(&href).ok()?;
    (url.origin() == base.origin()
        && url.path() == base.path()
        && url.query_pairs().any(|(key, _)| key == "cursor"))
    .then_some(url)
}

fn fetch_tibo_timeline() -> Result<TiboTimeline, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|error| format!("Could not prepare X timeline request: {error}"))?;
    let mut failures = Vec::new();
    match client
        .get(TIBO_NITTER_URL)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/140.0 Safari/537.36",
        )
        .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .send()
    {
        Ok(response) if response.status().is_success() => match response.text() {
            Ok(html) => match parse_tibo_nitter_posts(&html) {
                Ok(mut posts) => {
                    let now = Utc::now().timestamp();
                    let mut page = html;
                    let mut visited = Vec::new();
                    // Continue through history until the page reaches the 24-hour boundary.
                    for _ in 0..10 {
                        if parse_tibo_nitter_posts_impl(&page, true).is_ok_and(|regular| regular.iter().filter_map(social_post_timestamp).any(|timestamp| timestamp <= now - 86400)) { break; }
                        let Some(next) = nitter_next_page(&page) else { break; };
                        if visited.contains(&next) { break; }
                        visited.push(next.clone());
                        let next_page = client.get(next)
                            .header(reqwest::header::USER_AGENT, "Mozilla/5.0")
                            .send().and_then(|response| response.error_for_status()).and_then(|response| response.text());
                        match next_page {
                            Ok(html) => match parse_tibo_nitter_posts(&html) {
                                Ok(older) => { posts.extend(older); page = html; }
                                Err(_) => break,
                            },
                            Err(_) => break,
                        }
                    }
                    return summarize_tibo_timeline(posts, now);
                },
                Err(error) => failures.push(error),
            },
            Err(error) => failures.push(error.to_string()),
        },
        Ok(response) => failures.push(format!("mirror HTTP {}", response.status())),
        Err(error) => failures.push(error.to_string()),
    }
    for url in TIBO_TIMELINE_URLS {
        let response = match client
            .get(url)
            .header(
                reqwest::header::USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/140.0 Safari/537.36",
            )
            .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
            .send()
        {
            Ok(response) => response,
            Err(error) => {
                failures.push(error.to_string());
                continue;
            }
        };
        let status = response.status();
        if !status.is_success() {
            failures.push(format!("HTTP {status}"));
            continue;
        }
        match response.text() {
            Ok(html) => match parse_tibo_posts(&html) {
                Ok(posts) => return summarize_tibo_timeline(posts, Utc::now().timestamp()),
                Err(error) => failures.push(error),
            },
            Err(error) => failures.push(error.to_string()),
        }
    }
    Err(format!(
        "Could not load Tibo's X timeline: {}",
        failures.join("; ")
    ))
}

#[tauri::command]
async fn get_tibo_timeline() -> Result<TiboTimeline, String> {
    tauri::async_runtime::spawn_blocking(fetch_tibo_timeline)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn open_tibo_timeline() -> Result<CommandResult, String> {
    open_url("https://x.com/thsottiaux")
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
fn get_app_settings() -> Result<AppSettings, String> {
    let _guard = APP_SETTINGS_LOCK
        .lock()
        .map_err(|_| "App settings are locked.")?;
    load_settings()
}

#[tauri::command]
fn get_app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
fn set_app_language(language: String) -> Result<AppSettings, String> {
    let _guard = APP_SETTINGS_LOCK
        .lock()
        .map_err(|_| "App settings are locked.")?;
    let mut settings = load_settings()?;
    settings.language = normalize_language(&language);
    save_settings(&settings)?;
    Ok(settings)
}

#[tauri::command]
fn set_sync_local_projects(enabled: bool) -> Result<AppSettings, String> {
    let _guard = APP_SETTINGS_LOCK
        .lock()
        .map_err(|_| "App settings are locked.")?;
    let mut settings = load_settings()?;
    settings.sync_local_projects = enabled;
    save_settings(&settings)?;
    Ok(settings)
}

#[tauri::command]
fn get_cloudbase_status() -> Result<cloudbase::CloudBaseStatus, String> {
    cloudbase::status(&locate_data_dir())
}

#[tauri::command]
async fn cloudbase_bootstrap() -> Result<Value, String> {
    let data_dir = locate_data_dir();
    tauri::async_runtime::spawn_blocking(move || {
        let installation_id = cloudbase::installation_id(&data_dir)?;
        cloudbase::invoke(
            &data_dir,
            cloudbase::FunctionKind::Client,
            serde_json::json!({
                "action": "bootstrap",
                "installationId": installation_id,
                "appVersion": env!("CARGO_PKG_VERSION")
            }),
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn cloudbase_create_order_draft(
    product_id: String,
    client_order_id: String,
) -> Result<Value, String> {
    let data_dir = locate_data_dir();
    tauri::async_runtime::spawn_blocking(move || {
        let installation_id = cloudbase::installation_id(&data_dir)?;
        cloudbase::invoke(
            &data_dir,
            cloudbase::FunctionKind::Client,
            serde_json::json!({
                "action": "createOrderDraft",
                "installationId": installation_id,
                "productId": product_id,
                "clientOrderId": client_order_id,
                "appVersion": env!("CARGO_PKG_VERSION")
            }),
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn cloudbase_attach_payment_reference(
    order_id: String,
    trade_no: String,
) -> Result<Value, String> {
    let data_dir = locate_data_dir();
    tauri::async_runtime::spawn_blocking(move || {
        cloudbase::invoke(
            &data_dir,
            cloudbase::FunctionKind::Client,
            serde_json::json!({
                "action": "attachPaymentReference",
                "orderId": order_id,
                "tradeNo": trade_no
            }),
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn cloudbase_get_order(order_id: String) -> Result<Value, String> {
    let data_dir = locate_data_dir();
    tauri::async_runtime::spawn_blocking(move || {
        cloudbase::invoke(
            &data_dir,
            cloudbase::FunctionKind::Client,
            serde_json::json!({
                "action": "getOrder",
                "orderId": order_id
            }),
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn cloudbase_reconcile_payment(order_id: String) -> Result<Value, String> {
    let data_dir = locate_data_dir();
    tauri::async_runtime::spawn_blocking(move || {
        let installation_id = cloudbase::installation_id(&data_dir)?;
        cloudbase::invoke(
            &data_dir,
            cloudbase::FunctionKind::Client,
            serde_json::json!({
                "action": "reconcilePayment",
                "installationId": installation_id,
                "orderId": order_id
            }),
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn cloudbase_abandon_order(order_id: String) -> Result<Value, String> {
    let data_dir = locate_data_dir();
    tauri::async_runtime::spawn_blocking(move || {
        let installation_id = cloudbase::installation_id(&data_dir)?;
        cloudbase::invoke(
            &data_dir,
            cloudbase::FunctionKind::Client,
            serde_json::json!({
                "action": "abandonOrder",
                "installationId": installation_id,
                "orderId": order_id
            }),
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

fn sha256_hex(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn install_cloud_delivery(delivery: &Value) -> Result<Option<String>, String> {
    let allocation_id = value_string(delivery, "allocationId")
        .ok_or_else(|| "Cloud delivery does not contain an allocation ID.".to_string())?;
    let mut db = load_db()?;
    if db
        .profiles
        .values()
        .any(|item| item.allocation_id.as_deref() == Some(allocation_id.as_str()))
    {
        return Ok(None);
    }

    let auth_text = value_string(delivery, "authText")
        .ok_or_else(|| "Cloud delivery does not contain auth.json.".to_string())?;
    let config_text = value_string(delivery, "configText")
        .ok_or_else(|| "Cloud delivery does not contain config.toml.".to_string())?;
    core::auth_files::validate(&auth_text, Some(&config_text))?;
    let expected_auth_hash = value_string(delivery, "authSha256")
        .ok_or_else(|| "Cloud delivery is missing the auth.json checksum.".to_string())?;
    let expected_config_hash = value_string(delivery, "configSha256")
        .ok_or_else(|| "Cloud delivery is missing the config.toml checksum.".to_string())?;
    if sha256_hex(&auth_text) != expected_auth_hash.to_ascii_lowercase()
        || sha256_hex(&config_text) != expected_config_hash.to_ascii_lowercase()
    {
        return Err("Delivered profile checksum verification failed.".to_string());
    }

    let requested_name = value_string(delivery, "profileName")
        .unwrap_or_else(|| "Purchased Codex Profile".to_string());
    let mut profile_name = unique_profile_name(&requested_name)?;
    let mut destination = profiles_dir().join(&profile_name);
    if destination.exists() {
        profile_name = unique_profile_name(&format!("{}_{}", requested_name, now_stamp()))?;
        destination = profiles_dir().join(&profile_name);
    }
    let temporary = profiles_dir().join(format!(
        ".delivery-{}-{}",
        safe_profile_name(&profile_name),
        Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&temporary).map_err(|error| error.to_string())?;
    let write_result = (|| -> Result<(), String> {
        fs::write(temporary.join(AUTH_FILE), auth_text.as_bytes())
            .map_err(|error| error.to_string())?;
        fs::write(temporary.join(CONFIG_FILE), config_text.as_bytes())
            .map_err(|error| error.to_string())?;
        fs::rename(&temporary, &destination).map_err(|error| error.to_string())?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_dir_all(&temporary);
        return Err(format!("Failed to install delivered profile: {error}"));
    }

    let now = now_text();
    db.profiles.insert(
        profile_name.clone(),
        ProfileMeta {
            name: profile_name.clone(),
            created_at: now.clone(),
            updated_at: now,
            source: Some("purchase".to_string()),
            product_id: value_string(delivery, "productId"),
            order_id: value_string(delivery, "orderId"),
            allocation_id: Some(allocation_id),
            entitlement_expires_at: value_string(delivery, "expiresAt"),
            subscription_checked_at: None,
            subscription_active_until: None,
            web_session_id: None,
        },
    );
    if let Err(error) = save_db(&db) {
        let _ = fs::remove_dir_all(&destination);
        return Err(format!("Failed to record delivered profile: {error}"));
    }
    Ok(Some(profile_name))
}

#[tauri::command]
async fn cloudbase_sync_deliveries(
    state: State<'_, RuntimeState>,
) -> Result<DeliverySyncResult, String> {
    let data_dir = locate_data_dir();
    let delivered_profiles = tauri::async_runtime::spawn_blocking(move || {
        let installation_id = cloudbase::installation_id(&data_dir)?;
        let entitlements = cloudbase::invoke(
            &data_dir,
            cloudbase::FunctionKind::Client,
            serde_json::json!({
                "action": "syncEntitlements",
                "installationId": installation_id.clone()
            }),
        )?;
        let allocations = entitlements
            .get("allocations")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut delivered = Vec::new();
        let mut errors = Vec::new();
        for allocation in allocations {
            if value_string(&allocation, "status").as_deref() != Some("active") {
                continue;
            }
            let Some(allocation_id) = value_string(&allocation, "id") else {
                continue;
            };
            let already_installed = load_db()?
                .profiles
                .values()
                .any(|item| item.allocation_id.as_deref() == Some(allocation_id.as_str()));
            if already_installed {
                continue;
            }
            match cloudbase::invoke(
                &data_dir,
                cloudbase::FunctionKind::Client,
                serde_json::json!({
                    "action": "claimDelivery",
                    "installationId": installation_id.clone(),
                    "allocationId": allocation_id
                }),
            )
            .and_then(|result| {
                result
                    .get("delivery")
                    .ok_or_else(|| "Cloud delivery response is incomplete.".to_string())
                    .and_then(install_cloud_delivery)
            }) {
                Ok(Some(profile_name)) => delivered.push(profile_name),
                Ok(None) => {}
                Err(error) => errors.push(format!("{allocation_id}: {error}")),
            }
        }
        Ok::<_, String>((delivered, errors))
    })
    .await
    .map_err(|error| format!("Delivery synchronization task failed: {error}"))??;

    Ok(DeliverySyncResult {
        delivered_profiles: delivered_profiles.0,
        errors: delivered_profiles.1,
        snapshot: app_snapshot(Some(state))?,
    })
}

#[tauri::command]
async fn cloudbase_admin_login(password: String) -> Result<Value, String> {
    let data_dir = locate_data_dir();
    tauri::async_runtime::spawn_blocking(move || {
        cloudbase::invoke(
            &data_dir,
            cloudbase::FunctionKind::Admin,
            serde_json::json!({
                "action": "login",
                "password": password
            }),
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn cloudbase_admin_inventory(admin_token: String) -> Result<Value, String> {
    let data_dir = locate_data_dir();
    tauri::async_runtime::spawn_blocking(move || {
        cloudbase::invoke(
            &data_dir,
            cloudbase::FunctionKind::Admin,
            serde_json::json!({
                "action": "listInventory",
                "adminToken": admin_token
            }),
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn cloudbase_admin_upload_profile(
    app: tauri::AppHandle,
    admin_token: String,
    profile_name: String,
    eligible_products: Vec<String>,
    notes: Option<String>,
) -> Result<Value, String> {
    let data_dir = locate_data_dir();
    tauri::async_runtime::spawn_blocking(move || {
        let db = load_db()?;
        let meta = db
            .profiles
            .get(&profile_name)
            .ok_or_else(|| format!("Profile '{profile_name}' does not exist."))?;
        let view = view_for_profile(meta, db.active.as_ref());
        let usage = usage_for_profile(&app, &profile_name).ok();
        let primary_left = usage.as_ref().and_then(|value| value.primary_left);
        let secondary_left = usage.as_ref().and_then(|value| value.secondary_left);
        let reset_at = usage
            .as_ref()
            .map(|value| value.reset.clone())
            .unwrap_or_else(|| view.reset.clone());
        let plan = usage
            .as_ref()
            .map(|value| value.plan.clone())
            .filter(|value| !value.trim().is_empty() && value != "-")
            .unwrap_or_else(|| view.plan.clone());

        cloudbase::invoke(
            &data_dir,
            cloudbase::FunctionKind::Admin,
            serde_json::json!({
                "action": "uploadProfile",
                "adminToken": admin_token,
                "eligibleProducts": eligible_products,
                "notes": notes.unwrap_or_default(),
                "profile": {
                    "profileName": view.name,
                    "accountId": view.account_id,
                    "email": view.email,
                    "plan": plan,
                    "primaryLeft": primary_left,
                    "secondaryLeft": secondary_left,
                    "resetAt": reset_at,
                    "profileUpdatedAt": view.updated,
                    "authText": view.auth_text,
                    "configText": view.config_text
                }
            }),
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn cloudbase_admin_move_profile_pool(
    admin_token: String,
    profile_id: String,
    target_product_id: String,
) -> Result<Value, String> {
    let data_dir = locate_data_dir();
    tauri::async_runtime::spawn_blocking(move || {
        cloudbase::invoke(
            &data_dir,
            cloudbase::FunctionKind::Admin,
            serde_json::json!({
                "action": "moveInventoryPool",
                "adminToken": admin_token,
                "profileId": profile_id,
                "targetProductId": target_product_id
            }),
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn cloudbase_admin_logout(admin_token: String) -> Result<Value, String> {
    let data_dir = locate_data_dir();
    tauri::async_runtime::spawn_blocking(move || {
        cloudbase::invoke(
            &data_dir,
            cloudbase::FunctionKind::Admin,
            serde_json::json!({
                "action": "logout",
                "adminToken": admin_token
            }),
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

fn capture_switch_projects(
    target_profile: &str,
) -> Result<Option<project_sync::ProjectSync>, String> {
    let source = project_renderer_owner(&current_file(AUTH_FILE))?;
    let target = project_renderer_owner(&profile_path(target_profile).join(AUTH_FILE))?;
    project_sync::capture(&codex_dir(), &source, &target)
}

fn restore_switch_projects(
    snapshot: &project_sync::ProjectSync,
) -> Result<Option<project_sync::ProjectSyncResult>, String> {
    let target = project_renderer_owner(&current_file(AUTH_FILE))?;
    if platform::app_is_running()? {
        return Err("Codex is running, so its project settings were not changed.".to_string());
    }
    project_sync::restore(&codex_dir(), snapshot, &target)
}

fn resume_pending_switch(runtime: &RuntimeState) -> Result<bool, String> {
    let Some(mut record) = load_switch_recovery()? else {
        return Ok(false);
    };
    if !profile_files_complete(&record.target_profile) {
        return Err(format!(
            "Cannot recover profile switch because '{}' is incomplete.",
            record.target_profile
        ));
    }

    append_diagnostic_log(&format!(
        "resuming profile switch stage={} from={:?} target={}",
        record.stage, record.from_profile, record.target_profile
    ));

    if record.stage == "requested" {
        hard_stop_codex_app()?;
        set_switch_stage(&mut record, "codex-stopped")?;
    }
    if record.stage == "codex-stopped" {
        let _ = backup_current("before-switch", false)?;
        if record.sync_local_projects {
            match capture_switch_projects(&record.target_profile) {
                Ok(projects) => record.project_sync = projects,
                Err(error) => {
                    append_diagnostic_log(&format!("project sync capture skipped: {error}"))
                }
            }
        }
        let db = load_db()?;
        let source_matches = record.from_profile.as_deref().is_some_and(|name| {
            let expected = session_from_auth(&profile_path(name).join(AUTH_FILE)).account_id;
            expected.is_some() && expected == session_from_auth(&current_file(AUTH_FILE)).account_id
        });
        append_diagnostic_log(&format!(
            "profile switch source identity verified={} from={:?} target={}",
            source_matches, record.from_profile, record.target_profile
        ));
        if let Some(active_name) = record.from_profile.as_deref() {
            if db.profiles.contains_key(active_name) && current_files_complete() {
                save_current_to_profile(active_name)?;
            }
        }
        set_switch_stage(&mut record, "current-saved")?;
    }
    if record.stage == "current-saved" {
        let expected = session_from_auth(&profile_path(&record.target_profile).join(AUTH_FILE))
            .account_id
            .ok_or("Target profile has no verifiable account identity.")?;
        restore_profile(&record.target_profile)?;
        let raw = fs::read_to_string(current_file(AUTH_FILE)).map_err(|error| error.to_string())?;
        let auth: Value = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
        validate_auth_account(&auth, &expected)?;
        append_diagnostic_log(&format!(
            "profile switch target identity verified=true target={}",
            record.target_profile
        ));
        set_switch_stage(&mut record, "target-restored")?;
    }
    if record.stage == "target-restored" && record.sync_local_projects {
        if let Some(snapshot) = record.project_sync.as_ref() {
            match restore_switch_projects(snapshot) {
                Ok(Some(result)) => append_diagnostic_log(&format!(
                    "project sync restored {} local projects and {} local chat bindings",
                    result.projects, result.threads
                )),
                Ok(None) => append_diagnostic_log(
                    "project sync found no missing local project or chat bindings",
                ),
                Err(error) => {
                    append_diagnostic_log(&format!("project sync restore skipped: {error}"))
                }
            }
        }
        set_switch_stage(&mut record, "projects-checked")?;
    }
    if !matches!(
        record.stage.as_str(),
        "target-restored" | "projects-checked"
    ) {
        return Err(format!("Unknown profile switch stage '{}'.", record.stage));
    }

    thread::sleep(Duration::from_millis(500));
    start_codex_app(runtime)?;
    append_diagnostic_log(&format!(
        "profile switch completed target={}",
        record.target_profile
    ));
    clear_switch_recovery()?;
    Ok(true)
}

#[tauri::command]
async fn switch_profile(
    app: tauri::AppHandle,
    profile_name: String,
) -> Result<AppSnapshot, String> {
    let worker_app = app.clone();
    let requested_profile = profile_name.clone();
    let switch_result = tauri::async_runtime::spawn_blocking(move || {
        let runtime = worker_app.state::<RuntimeState>();
        // Switching must not overlap usage auth writeback or isolated CLI probes.
        append_diagnostic_log(&format!(
            "profile switch waiting for account operation lock target={requested_profile}"
        ));
        let _guard = runtime
            .subscription_refresh_lock
            .lock()
            .map_err(|_| "Account operation lock is unavailable.".to_string())?;
        append_diagnostic_log(&format!(
            "profile switch account operation lock acquired target={requested_profile}"
        ));
        let db = load_db()?;
        if !db.profiles.contains_key(&requested_profile) {
            return Err(format!("Profile '{}' was not found.", requested_profile));
        }
        if !profile_files_complete(&requested_profile) {
            return Err(format!("Profile '{}' is incomplete.", requested_profile));
        }
        if db.active.as_deref() == Some(requested_profile.as_str()) {
            return Ok(());
        }

        let record = SwitchRecoveryRecord {
            from_profile: db.active,
            target_profile: requested_profile,
            stage: "requested".to_string(),
            started_at: Utc::now().to_rfc3339(),
            sync_local_projects: get_app_settings()
                .map(|settings| settings.sync_local_projects)
                .unwrap_or_else(|error| {
                    append_diagnostic_log(&format!("project sync settings unavailable: {error}"));
                    false
                }),
            project_sync: None,
        };
        save_switch_recovery(&record)?;
        append_diagnostic_log(&format!(
            "profile switch requested from={:?} target={}",
            record.from_profile, record.target_profile
        ));
        match resume_pending_switch(&runtime) {
            Ok(_) => Ok(()),
            Err(first_error) => {
                append_diagnostic_log(&format!(
                    "profile switch first attempt failed; retrying: {first_error}"
                ));
                match resume_pending_switch(&runtime) {
                    Ok(_) => Ok(()),
                    Err(retry_error) => {
                        // Never leave Codex silently closed after a recoverable command error.
                        let restart_result = start_codex_app(&runtime);
                        Err(format!(
                            "{first_error}; retry failed: {retry_error}; fallback restart: {:?}",
                            restart_result.err()
                        ))
                    }
                }
            }
        }
    })
    .await
    .map_err(|error| {
        let message = format!("Profile switch worker failed: {error}");
        append_diagnostic_log(&message);
        message
    })?;

    if let Err(error) = switch_result {
        append_diagnostic_log(&format!(
            "profile switch failed target={} error={}",
            profile_name, error
        ));
        return Err(error);
    }
    app_snapshot(Some(app.state::<RuntimeState>()))
}

#[tauri::command]
async fn login_new_account(
    app: tauri::AppHandle,
    state: State<'_, RuntimeState>,
) -> Result<CommandResult, String> {
    core::auth_files::check_store(&codex_dir())?;
    let run_id = Uuid::new_v4().simple().to_string();
    {
        let mut login = state
            .managed_login
            .lock()
            .map_err(|_| "Managed login state is unavailable.".to_string())?;
        *login = ManagedLoginRuntime {
            run_id: Some(run_id.clone()),
            message: "Opening the ChatGPT sign-in session...".to_string(),
            ..ManagedLoginRuntime::default()
        };
    }
    open_managed_login_window(
        &app,
        "https://chatgpt.com/auth/login",
        &run_id,
        Arc::clone(&state.managed_login),
    )?;
    start_web_first_login(app, run_id, Arc::clone(&state.managed_login));
    Ok(CommandResult::ok(
        "Managed web-first login started. Your current Codex session will remain untouched until the ChatGPT web session is confirmed.",
    ))
}

#[tauri::command]
fn get_login_state(state: State<'_, RuntimeState>) -> LoginState {
    let mut file_state = current_login_state();
    if let Ok(login) = state.managed_login.lock() {
        if login.run_id.is_some() {
            if !login.message.is_empty() {
                file_state.message = login.message.clone();
            }
            file_state.terminal = login.terminal;
            file_state.failed = login.failed;
            if file_state.ready && (!login.completed || !login.web_session_finished) {
                file_state.ready = false;
                file_state.signature = None;
            }
        }
    }
    file_state
}

#[tauri::command]
fn abort_login_new_account(
    app: tauri::AppHandle,
    state: State<'_, RuntimeState>,
) -> Result<CommandResult, String> {
    let run_id = state
        .managed_login
        .lock()
        .map_err(|_| "Managed login state is unavailable.".to_string())?
        .run_id
        .clone()
        .ok_or_else(|| "No login flow is active.".to_string())?;
    let message = cancel_managed_login(
        &app,
        &state.managed_login,
        &run_id,
        "Login cancelled by the user.",
        false,
        true,
    )?;
    Ok(CommandResult::ok(message))
}

#[tauri::command]
async fn refresh_profile_usage(
    app: tauri::AppHandle,
    state: State<'_, RuntimeState>,
    profile_name: String,
) -> Result<ProfileUsage, String> {
    let task_name = profile_name.clone();
    let subscription_refresh_lock = Arc::clone(&state.subscription_refresh_lock);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = subscription_refresh_lock
            .lock()
            .map_err(|_| "Subscription refresh lock is unavailable.".to_string())?;
        usage_for_profile(&app, &profile_name)
    })
    .await
    .map_err(|error| format!("Usage refresh task failed for {task_name}: {error}"))?
}

#[tauri::command]
async fn start_five_hour_window(
    app: tauri::AppHandle,
    state: State<'_, RuntimeState>,
    profile_name: String,
    request_id: String,
) -> Result<StartFiveHourResult, String> {
    let trace = FiveHourTrace::new(&request_id, &profile_name)?;
    trace.log("backend_received", serde_json::json!({}));
    let refresh_lock = Arc::clone(&state.subscription_refresh_lock);
    tauri::async_runtime::spawn_blocking(move || {
        trace.log("refresh_lock_wait", serde_json::json!({}));
        let started = std::time::Instant::now();
        let _guard = refresh_lock
            .lock()
            .map_err(|_| "Usage refresh lock is unavailable.".to_string())?;
        trace.log("refresh_lock_acquired", serde_json::json!({"waitMs": started.elapsed().as_millis()}));
        #[cfg(windows)]
        {
            let mut path_command = Command::new("where.exe");
            path_command.arg("codex.cmd").creation_flags(CREATE_NO_WINDOW);
            if let Ok(output) = path_command.output() {
                trace.log("cli_path", serde_json::json!({"paths": sanitize_five_hour_text(&String::from_utf8_lossy(&output.stdout)), "exitCode": output.status.code()}));
            }
        }
        let mut version_command = platform::cli_command();
        version_command.arg("--version").stdin(Stdio::null());
        #[cfg(windows)]
        version_command.creation_flags(CREATE_NO_WINDOW);
        match version_command.output() {
            Ok(output) => trace.log("cli_environment", serde_json::json!({"version": sanitize_five_hour_text(&String::from_utf8_lossy(&output.stdout)), "exitCode": output.status.code(), "stderr": sanitize_five_hour_text(&String::from_utf8_lossy(&output.stderr)), "proxyEnvironmentPresent": (["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"].iter().any(|key| env::var_os(key).is_some()))})),
            Err(error) => trace.log("cli_environment_failed", serde_json::json!({"error": sanitize_five_hour_text(&error.to_string())})),
        }
        let result = start_five_hour_window_for_profile(&app, &profile_name, &trace);
        match &result {
            Ok(result) => trace.log("request_finished", serde_json::json!({"confirmed": result.confirmed, "alreadyActive": result.already_active, "chatCleanup": result.chat_cleanup})),
            Err(error) => trace.log("request_failed", serde_json::json!({"error": sanitize_five_hour_text(error)})),
        }
        append_diagnostic_log(&format!("five-hour request={} profile={} success={} log=diagnostics/five-hour-requests/{}.jsonl", trace.id, profile_name, result.as_ref().is_ok_and(|result| result.confirmed), trace.id));
        result.map_err(|error| format!("{error} [diagnostic: {}]", trace.id))
    })
    .await
    .map_err(|error| format!("Five-hour request task failed: {error}"))?
}

#[tauri::command]
fn finish_login_new_account(
    app: tauri::AppHandle,
    state: State<'_, RuntimeState>,
) -> Result<AppSnapshot, String> {
    let result = (|| -> Result<AppSnapshot, String> {
        let state_now = current_login_state();
        if !state_now.ready {
            return Err(state_now.message);
        }
        let session = session_from_auth(&current_file(AUTH_FILE));
        let base = session
            .name
            .clone()
            .or(session.email.clone())
            .or(session.account_id.clone())
            .unwrap_or_else(|| format!("Codex_Profile_{}", now_stamp()));
        let name = unique_profile_name(&base)?;
        set_pending_login_profile_name(&name)?;
        save_current_to_profile(&name)?;
        let managed_result = state.managed_login.lock().ok().and_then(|login| {
            login.run_id.clone().map(|run_id| {
                (
                    run_id,
                    login.web_session_ready,
                    login.subscription_active_until.clone(),
                )
            })
        });
        if let Some((run_id, web_session_ready, active_until)) = managed_result {
            let mut db = load_db()?;
            if let Some(meta) = db.profiles.get_mut(&name) {
                meta.web_session_id = web_session_ready.then_some(run_id);
                meta.subscription_active_until = active_until.or(session.subscription_active_until);
                meta.subscription_checked_at = web_session_ready.then(|| Utc::now().to_rfc3339());
            }
            save_db(&db)?;
        }
        clear_login_recovery()?;
        if let Ok(mut login) = state.managed_login.lock() {
            login.terminal = true;
            login.profile_saved = true;
            login.message = format!("New profile '{name}' was saved successfully.");
        }
        app_snapshot(Some(state.clone()))
    })();

    match result {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => {
            let run_id = state
                .managed_login
                .lock()
                .ok()
                .and_then(|login| login.run_id.clone());
            if let Some(run_id) = run_id {
                let recovery = cancel_managed_login(
                    &app,
                    &state.managed_login,
                    &run_id,
                    &format!("The new profile could not be saved: {error}"),
                    true,
                    true,
                )
                .unwrap_or_else(|restore_error| format!("Recovery also failed: {restore_error}"));
                return Err(recovery);
            }
            Err(error)
        }
    }
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
        return Err(format!("Profile '{profile_name}' does not exist."));
    }
    let src = profile_path(&profile_name);
    if src.exists() {
        let trash = trash_dir();
        fs::create_dir_all(&trash).map_err(|error| {
            format!("Failed to create trash dir '{}': {error}", trash.display())
        })?;
        let mut dest = trash.join(format!(
            "{}_{}",
            safe_profile_name(&profile_name),
            now_stamp()
        ));
        let mut duplicate_index = 1;
        while dest.exists() {
            dest = trash.join(format!(
                "{}_{}_{}",
                safe_profile_name(&profile_name),
                now_stamp(),
                duplicate_index
            ));
            duplicate_index += 1;
        }
        fs::rename(&src, &dest)
            .or_else(|_| {
                fs::create_dir_all(&dest)?;
                for entry in fs::read_dir(&src)? {
                    let entry = entry?;
                    fs::rename(entry.path(), dest.join(entry.file_name()))?;
                }
                fs::remove_dir_all(&src)
            })
            .map_err(|error| {
                format!(
                    "Failed to move profile '{}' from '{}' to '{}': {error}",
                    profile_name,
                    src.display(),
                    dest.display()
                )
            })?;
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

fn resolve_store_payment_product(
    goods_key: Option<&str>,
    item_url: Option<&str>,
) -> StorePaymentProduct {
    let key = goods_key.unwrap_or_default().trim();
    let url = item_url.unwrap_or_default().trim();
    if key == CATFK_WEEKLY_GOODS_KEY || url == STORE_WEEKLY_PAYMENT_URL {
        return StorePaymentProduct {
            goods_key: CATFK_WEEKLY_GOODS_KEY,
            item_url: STORE_WEEKLY_PAYMENT_URL,
        };
    }
    StorePaymentProduct {
        goods_key: CATFK_MONTHLY_GOODS_KEY,
        item_url: STORE_MONTHLY_PAYMENT_URL,
    }
}

fn run_store_payment_script(
    fetch_cashier: bool,
    product: &StorePaymentProduct,
) -> Result<Value, String> {
    services::http::payment(fetch_cashier, product)
}

#[tauri::command]
async fn create_store_payment_order(
    goods_key: Option<String>,
    item_url: Option<String>,
) -> Result<PaymentOrderResult, String> {
    let product = resolve_store_payment_product(goods_key.as_deref(), item_url.as_deref());
    let result = run_store_payment_script(false, &product)?;
    Ok(PaymentOrderResult {
        ok: true,
        message: "Payment order created.".to_string(),
        trade_no: result
            .get("tradeNo")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        total_amount: result
            .get("totalAmount")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        payurl: result
            .get("payurl")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        cashier_html: None,
    })
}

#[tauri::command]
async fn capture_store_payment_qr(
    app: tauri::AppHandle,
    trade_no: String,
    total_amount: Option<String>,
    payurl: String,
) -> Result<PaymentStartResult, String> {
    let payurl = payurl.trim().to_string();
    if payurl.is_empty() {
        return Err("Payment URL is empty.".to_string());
    }
    let mut capture_debug = None;
    let qr_image = match capture_qr_from_rendered_page(&app, &payurl) {
        Ok(data_uri) => Some(data_uri),
        Err(error) => {
            capture_debug = Some(error);
            None
        }
    };
    Ok(PaymentStartResult {
        ok: true,
        message: "Payment QR code fetched.".to_string(),
        trade_no,
        total_amount,
        payurl,
        qr_image,
        capture_debug,
    })
}

#[tauri::command]
async fn start_store_payment(app: tauri::AppHandle) -> Result<PaymentStartResult, String> {
    let product = resolve_store_payment_product(None, None);
    let result = run_store_payment_script(true, &product)?;
    let payurl = result
        .get("payurl")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let cashier_html = result
        .get("cashierHtml")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut capture_debug = None;
    let qr_image = extract_qr_image(cashier_html, &payurl)
        .and_then(|candidate| fetch_image_as_data_uri(&candidate, &payurl))
        .or_else(|| match capture_qr_from_rendered_page(&app, &payurl) {
            Ok(data_uri) => Some(data_uri),
            Err(error) => {
                capture_debug = Some(error);
                None
            }
        });
    Ok(PaymentStartResult {
        ok: true,
        message: "Payment order created.".to_string(),
        trade_no: result
            .get("tradeNo")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        total_amount: result
            .get("totalAmount")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        payurl: payurl.clone(),
        qr_image,
        capture_debug,
    })
}

#[tauri::command]
fn open_payment_url(url: String) -> Result<CommandResult, String> {
    let target = if url.trim().is_empty() {
        STORE_MONTHLY_PAYMENT_URL
    } else {
        url.trim()
    };
    open_url(target)
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

fn set_fixed_window_size(
    window: &tauri::WebviewWindow,
    width: f64,
    height: f64,
) -> Result<(), String> {
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
    Ok(())
}

#[tauri::command]
fn resize_window(app: tauri::AppHandle, width: f64, height: f64) -> Result<CommandResult, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    if let (Ok(size), Ok(scale)) = (window.outer_size(), window.scale_factor()) {
        let requested_height = (height * scale).round() as u32;
        if size.height < requested_height / 2 {
            if let Ok(Some(monitor)) = window.current_monitor() {
                if let Ok(position) = window.outer_position() {
                    let _ = window
                        .set_position(PhysicalPosition::new(position.x, monitor.position().y));
                }
            }
        }
    }
    let _ = window.set_always_on_top(false);
    set_fixed_window_size(&window, width, height)?;

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
    hide_main_window(&app);

    Ok(CommandResult::ok("Window hidden to tray."))
}

#[tauri::command]
fn set_floating_bar(
    app: tauri::AppHandle,
    enabled: bool,
    title: String,
    usage: String,
    width: f64,
) -> Result<CommandResult, String> {
    {
        let runtime = app.state::<RuntimeState>();
        *runtime
            .floating_enabled
            .lock()
            .map_err(|_| "floating bar state is locked.".to_string())? = enabled;
        *runtime
            .floating_payload
            .lock()
            .map_err(|_| "floating bar payload is locked.".to_string())? = FloatingBarPayload {
            title,
            usage,
            width,
        };
    }
    if enabled {
        platform::floating::start(&app, &floating_bar_payload(&app))?;
    } else {
        hide_floating_bar_window(&app);
    }
    Ok(CommandResult::ok("Floating bar updated."))
}

#[tauri::command]
fn restore_main_window(app: tauri::AppHandle) -> Result<CommandResult, String> {
    show_main_window(&app);
    Ok(CommandResult::ok("Window restored."))
}

#[tauri::command]
fn get_floating_bar(app: tauri::AppHandle) -> FloatingBarPayload {
    floating_bar_payload(&app)
}

pub fn run() {
    install_panic_reporter();
    configure_native_crash_dumps();
    append_diagnostic_log("application starting");
    let Some(instance_listener) = acquire_instance_listener() else {
        append_diagnostic_log("secondary instance handed off to the running instance");
        return;
    };

    let run_result = tauri::Builder::default()
        .manage(RuntimeState {
            launch_target: Mutex::new(find_shell_app()),
            tray_profiles: Mutex::new(Vec::new()),
            floating_enabled: Mutex::new(false),
            floating_payload: Mutex::new(FloatingBarPayload::default()),
            subscription_refresh_lock: Arc::new(Mutex::new(())),
            managed_login: Arc::new(Mutex::new(ManagedLoginRuntime::default())),
        })
        .setup(move |app| {
            match restore_pending_login_recovery() {
                Ok(Some(record)) => {
                    remove_login_web_session_later(record.run_id);
                    let runtime = app.state::<RuntimeState>();
                    if let Err(error) = start_codex_app(&runtime) {
                        eprintln!(
                            "Interrupted login was restored, but Codex could not restart: {error}"
                        );
                    }
                }
                Ok(None) => {}
                Err(error) => eprintln!("Interrupted login recovery failed: {error}"),
            }
            {
                let runtime = app.state::<RuntimeState>();
                match resume_pending_switch(&runtime) {
                    Ok(true) => append_diagnostic_log("interrupted profile switch recovered"),
                    Ok(false) => {}
                    Err(error) => {
                        append_diagnostic_log(&format!(
                            "interrupted profile switch recovery failed: {error}"
                        ));
                        if let Err(restart_error) = start_codex_app(&runtime) {
                            append_diagnostic_log(&format!(
                                "fallback Codex restart failed: {restart_error}"
                            ));
                        }
                    }
                }
            }
            start_instance_wake_listener(app.handle().clone(), instance_listener);
            start_floating_bar_watchdog(app.handle().clone());
            let menu = build_tray_menu(app.handle(), &[])?;
            TrayIconBuilder::with_id(TRAY_ID)
                .tooltip("Codex Switch")
                .icon(APP_ICON)
                .menu(&menu)
                .show_menu_on_left_click(cfg!(target_os = "macos"))
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        recover_floating_bar(tray.app_handle());
                        toggle_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    // Native closes (taskbar menu, Alt+F4) match the custom close button.
                    api.prevent_close();
                    hide_main_window(window.app_handle());
                }
            }
        })
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "tray_toggle_window" {
                recover_floating_bar(app);
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
                append_diagnostic_log("tray exit requested");
                platform::floating::stop(app);
                if let Some(window) = app.get_webview_window(FLOATING_BAR_ID) {
                    let _ = window.hide();
                    let _ = window.close();
                }
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
            get_app_settings,
            get_app_version,
            commands::get_platform_info,
            set_app_language,
            set_sync_local_projects,
            get_tibo_timeline,
            open_tibo_timeline,
            get_cloudbase_status,
            cloudbase_bootstrap,
            cloudbase_create_order_draft,
            cloudbase_attach_payment_reference,
            cloudbase_get_order,
            cloudbase_reconcile_payment,
            cloudbase_abandon_order,
            cloudbase_sync_deliveries,
            cloudbase_admin_login,
            cloudbase_admin_inventory,
            cloudbase_admin_upload_profile,
            cloudbase_admin_move_profile_pool,
            cloudbase_admin_logout,
            list_profiles,
            switch_profile,
            login_new_account,
            get_login_state,
            abort_login_new_account,
            refresh_profile_usage,
            start_five_hour_window,
            log_five_hour_event,
            get_five_hour_progress,
            finish_login_new_account,
            save_current_as_new,
            delete_profile,
            open_chatgpt_codex,
            open_data_dir,
            open_codex_dir,
            create_store_payment_order,
            capture_store_payment_qr,
            start_store_payment,
            open_payment_url,
            hard_kill_codex,
            update_tray_profiles,
            start_window_drag,
            show_side_panel,
            hide_side_panel,
            resize_window,
            minimize_window,
            close_window,
            set_floating_bar,
            restore_main_window,
            get_floating_bar
        ])
        .run(tauri::generate_context!());

    match run_result {
        Ok(()) => append_diagnostic_log("application exited normally"),
        Err(error) => {
            append_diagnostic_log(&format!("Tauri runtime failed: {error}"));
            panic!("error while running Codex Switch: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_writeback_rejects_desktop_switched_during_usage_query() {
        let dir = env::temp_dir().join(format!("csw-writeback-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let desktop = dir.join("desktop.json");
        let profile = dir.join("profile.json");
        let original = r#"{"tokens":{"account_id":"account-a","access_token":"old"}}"#;
        fs::write(&profile, original).unwrap();
        // Query started for A, but the desktop has already switched to B when it finishes.
        fs::write(
            &desktop,
            r#"{"tokens":{"account_id":"account-b","access_token":"new"}}"#,
        )
        .unwrap();
        assert!(copy_matching_auth(&desktop, &profile, "account-a").is_err());
        assert_eq!(fs::read_to_string(&profile).unwrap(), original);
        fs::write(
            &desktop,
            r#"{"tokens":{"account_id":"account-a","access_token":"refreshed"}}"#,
        )
        .unwrap();
        copy_matching_auth(&desktop, &profile, "account-a").unwrap();
        assert_eq!(
            session_from_auth(&profile).access_token.as_deref(),
            Some("refreshed")
        );
        assert!(copy_matching_auth(&desktop, &profile, "account-b").is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn account_identity_rejects_mixed_account_token_fields() {
        // JWT payload decodes to an account-b access token.
        let auth = serde_json::json!({"tokens": {
            "account_id": "account-a",
            "access_token": "e30.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjb3VudC1iIn19.signature"
        }});
        assert!(validate_auth_account(&auth, "account-a").is_err());
        assert!(validate_auth_account(&auth, "account-b").is_err());
        assert!(validate_auth_account(&serde_json::json!({}), "account-a").is_err());
    }

    #[test]
    fn five_hour_active_account_auth_is_never_written_back() {
        let dir = env::temp_dir().join(format!("csw-active-auth-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("stored.json");
        let probe = dir.join("isolated.json");
        let original = r#"{"auth_mode":"chatgpt","tokens":{"account_id":"account-a","access_token":"original"}}"#;
        let refreshed = r#"{"auth_mode":"chatgpt","tokens":{"account_id":"account-a","access_token":"refreshed"}}"#;
        fs::write(&source, original).unwrap();
        fs::write(&probe, refreshed).unwrap();
        assert!(!save_five_hour_probe_auth(&source, &probe, "account-a", true).unwrap());
        assert_eq!(fs::read_to_string(&source).unwrap(), original);
        assert!(save_five_hour_probe_auth(&source, &probe, "account-a", false).unwrap());
        assert_eq!(
            session_from_auth(&source).access_token.as_deref(),
            Some("refreshed")
        );
        assert!(save_five_hour_probe_auth(&source, &probe, "different-account", false).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn five_hour_windows_proxy_formats() {
        assert_eq!(
            parse_system_cli_proxy("127.0.0.1:7890").as_deref(),
            Some("http://127.0.0.1:7890")
        );
        assert_eq!(
            parse_system_cli_proxy("http=127.0.0.1:7890;https=127.0.0.1:7891").as_deref(),
            Some("http://127.0.0.1:7891")
        );
        assert_eq!(
            parse_system_cli_proxy("socks5://127.0.0.1:1080").as_deref(),
            Some("socks5://127.0.0.1:1080")
        );
        assert!(parse_system_cli_proxy("").is_none());
        assert!(parse_system_cli_proxy("ftp://127.0.0.1:21").is_none());
    }

    #[test]
    fn five_hour_diagnostic_redacts_credentials() {
        let text = sanitize_five_hour_text("HTTP 401 Bearer secret-token JWT eyJhbGci.eyJ1c2Vy.signature sk-example-key retry failed");
        assert!(!text.contains("secret-token"));
        assert!(!text.contains("eyJhbGci"));
        assert!(!text.contains("sk-example-key"));
        assert!(text.contains("HTTP 401"));
        assert!(text.contains("retry failed"));
        assert!(!sanitize_five_hour_text(
            "refresh_token=private-value access_token: another-secret"
        )
        .contains("private-value"));
        assert!(!sanitize_five_hour_text(
            "refresh_token=private-value access_token: another-secret"
        )
        .contains("another-secret"));
        assert!(sanitize_five_hour_text(&"x".repeat(10_000)).len() <= 1024);
    }

    #[test]
    fn five_hour_diagnostic_writes_correlated_local_events() {
        let id = Uuid::new_v4().to_string();
        let trace = FiveHourTrace::new(&id, "diagnostic self-test").unwrap();
        trace.log("self_test", serde_json::json!({"completed": true}));
        let path = diagnostics_dir()
            .join("five-hour-requests")
            .join(format!("{id}.jsonl"));
        let record: Value =
            serde_json::from_str(fs::read_to_string(&path).unwrap().trim()).unwrap();
        assert_eq!(record["requestId"], id);
        assert_eq!(record["stage"], "self_test");
        assert_eq!(record["details"]["completed"], true);
        assert!(FiveHourTrace::new("../invalid", "test").is_err());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn five_hour_reset_stability_confirms_zero_percent_usage() {
        let now = 1_800_000_100;
        let fixed_reset = now + FIVE_HOUR_WINDOW_SECONDS - 60;
        assert!(five_hour_reset_is_stable(
            fixed_reset,
            fixed_reset + 1,
            12,
            now
        ));
        assert!(!five_hour_reset_is_stable(
            fixed_reset,
            fixed_reset + 1,
            5,
            now
        ));
        assert!(!five_hour_reset_is_stable(
            now + FIVE_HOUR_WINDOW_SECONDS - 12,
            now + FIVE_HOUR_WINDOW_SECONDS,
            12,
            now,
        ));
    }

    fn test_post(text: &str, timestamp: i64) -> SocialPost {
        let id = ((timestamp as u64 * 1000 - 1_288_834_974_657) << 22) + 1;
        SocialPost {
            text: text.to_string(),
            url: format!("https://x.com/thsottiaux/status/{id}"),
            created_at: None,
        }
    }

    #[test]
    fn reset_history_survives_newer_unrelated_post_and_excludes_expired_posts() {
        let now = 1_791_700_000;
        let old = test_post("RESET tomorrow", now - 3600);
        let latest = test_post("Other news", now - 60);
        let result = summarize_tibo_timeline(vec![old, latest.clone()], now).unwrap();
        assert_eq!(result.latest_post, latest);
        assert_eq!(result.reset_post_at, Some(now - 3600));
        for timestamp in [now - 86400, now - 86401, now + 1] {
            let result = summarize_tibo_timeline(vec![test_post("reset", timestamp)], now).unwrap();
            assert_eq!(result.reset_post_at, None);
        }
        let result = summarize_tibo_timeline(vec![test_post("resets", now - 86399)], now).unwrap();
        assert_eq!(result.reset_post_at, Some(now - 86399));
    }

    #[test]
    fn timeline_parsers_retain_history_for_reset_detection() {
        let now = 1_791_700_000;
        let old = test_post("RESET soon", now - 3600);
        let latest = test_post("Other news", now - 60);
        let old_id = old.url.rsplit('/').next().unwrap();
        let latest_id = latest.url.rsplit('/').next().unwrap();
        let json = serde_json::json!({ "props": { "pageProps": { "timeline": { "entries": [
            { "id_str": old_id, "full_text": old.text }, { "id_str": latest_id, "full_text": latest.text }
        ] } } } });
        let html = format!("<script id=\"__NEXT_DATA__\">{json}</script>");
        let result = summarize_tibo_timeline(parse_tibo_posts(&html).unwrap(), now).unwrap();
        assert_eq!(result.reset_post_at, Some(now - 3600));
        assert_eq!(result.latest_post, latest);
        let html = format!(
            r#"<div class="timeline-item" data-username="thsottiaux"><a href="/thsottiaux/status/{latest_id}"></a><div class="tweet-content">Other news</div></div><div class="timeline-item" data-username="thsottiaux"><a href="/thsottiaux/status/{old_id}"></a><div class="tweet-content">RESET soon</div></div>"#
        );
        let result = summarize_tibo_timeline(parse_tibo_nitter_posts(&html).unwrap(), now).unwrap();
        assert_eq!(result.reset_post_at, Some(now - 3600));
        assert_eq!(result.latest_post, latest);
    }

    #[test]
    fn pagination_accepts_only_same_timeline_cursor_links() {
        let next = nitter_next_page(r#"<div class="show-more"><a href="/thsottiaux?cursor=abc&amp;foo=bar">Load more</a></div>"#).unwrap();
        assert_eq!(next.query(), Some("cursor=abc&foo=bar"));
        assert!(nitter_next_page(r#"<div class="show-more"><a href="https://other.test/thsottiaux?cursor=x">Next</a></div>"#).is_none());
    }

    #[test]
    fn latest_tibo_post_parser_ignores_older_pinned_entry() {
        let html = r#"<script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"timeline":{"entries":[{"content":{"tweet":{"id_str":"100","full_text":"Pinned old post"}}},{"content":{"tweet":{"id_str":"200","full_text":"Actual latest post"}}}]}}}}</script>"#;
        let post = parse_latest_tibo_post(html).expect("timeline should parse");
        assert_eq!(post.text, "Actual latest post");
        assert_eq!(post.url, "https://x.com/thsottiaux/status/200");
    }

    #[test]
    fn latest_tibo_nitter_parser_chooses_highest_status_id() {
        let html = r#"<div class="timeline-item" data-username="thsottiaux"><a class="tweet-link" href="/thsottiaux/status/300#m"></a><span class="tweet-date"><a title="Sep 2, 2026">now</a></span><div class="tweet-content media-body">Latest &amp; current</div></div><div class="timeline-item pinned" data-username="thsottiaux"><a class="tweet-link" href="/thsottiaux/status/100#m"></a><div class="tweet-content media-body">Pinned old post</div></div>"#;
        let post = parse_latest_tibo_nitter(html).expect("mirror timeline should parse");
        assert_eq!(post.text, "Latest & current");
        assert_eq!(post.url, "https://x.com/thsottiaux/status/300");
        assert_eq!(post.created_at.as_deref(), Some("Sep 2, 2026"));
    }

    #[test]
    #[ignore = "live X timeline smoke test"]
    fn latest_tibo_post_live_fetch() {
        let post = fetch_tibo_timeline()
            .expect("live timeline should return a post")
            .latest_post;
        assert!(!post.text.is_empty());
        assert!(post.url.starts_with("https://x.com/thsottiaux/status/"));
        eprintln!("{} | {}", post.url, post.text);
    }
}
