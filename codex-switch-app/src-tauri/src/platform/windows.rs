use super::{home_dir, AppTarget};
use std::{
    env,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub(crate) mod capture;
pub(super) mod floating;
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub(crate) fn run_powershell(script: &str) -> Result<String, String> {
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
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

pub(super) fn data_dir() -> PathBuf {
    // Keep the existing Windows location so upgrades never move profile data.
    env::var_os("LOCALAPPDATA")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join("AppData").join("Local"))
        .join("Codex Switch")
}

pub(super) fn find_app() -> Option<AppTarget> {
    let script = r#"
$apps = Get-StartApps -ErrorAction SilentlyContinue |
  Where-Object {
    ($_.Name -eq 'ChatGPT' -or $_.Name -eq 'Codex' -or $_.AppID -match '^OpenAI\.Codex_') -and
    $_.Name -ne 'Codex Switch' -and $_.AppID -ne 'com.codexswitch.app'
  } | Select-Object -First 1
if ($apps) { "$($apps.Name)|shell:AppsFolder\$($apps.AppID)" }
"#;
    let raw = run_powershell(script).ok()?;
    let (name, target) = raw.trim().split_once('|')?;
    Some(AppTarget {
        name: name.to_string(),
        target: target.to_string(),
    })
}

pub(super) fn app_is_running() -> Result<bool, String> {
    let raw = run_powershell(
        "@(Get-Process -Name ChatGPT,Codex,codex -ErrorAction SilentlyContinue).Count",
    )?;
    raw.trim()
        .parse::<u32>()
        .map(|n| n != 0)
        .map_err(|e| format!("Unable to verify Codex exit: {e}"))
}

pub(super) fn stop_app() -> Result<String, String> {
    // Do not use /T: this switcher can itself be a descendant of Codex.
    let script = r#"
$taskkill = Join-Path $env:SystemRoot 'System32\taskkill.exe'
& $taskkill /IM ChatGPT.exe /F | Out-Null
& $taskkill /IM Codex.exe /F | Out-Null
& $taskkill /IM codex.exe /F | Out-Null
Start-Sleep -Milliseconds 200
"#;
    let _ = run_powershell(script);
    if app_is_running()? {
        return Err("Codex is still running; login files were not changed.".into());
    }
    Ok("Codex stopped. Main process count after: 0.".into())
}

pub(super) fn start_app(app: &AppTarget) -> Result<String, String> {
    open_path(Path::new(&app.target))?;
    Ok(format!("App started: {} -> {}.", app.name, app.target))
}

pub(super) fn open_path(path: &Path) -> Result<(), String> {
    Command::new("explorer.exe")
        .arg(path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open {}: {e}", path.display()))
}

pub(super) fn open_url(url: &str) -> Result<(), String> {
    open_path(Path::new(url))
}

pub(super) fn cli_command() -> Command {
    let mut command =
        if let Some(path) = env::var_os("CODEX_SWITCH_CODEX_CLI").filter(|v| !v.is_empty()) {
            Command::new(path)
        } else {
            Command::new("codex.cmd")
        };
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

pub(super) fn terminate_process(process_id: u32) {
    if process_id <= 1 || process_id == std::process::id() {
        return;
    }
    let _ = Command::new("taskkill.exe")
        .args(["/PID", &process_id.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output();
}

pub(super) fn system_proxy() -> Option<String> {
    run_powershell("$p=Get-ItemProperty 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings'; if ($p.ProxyEnable -eq 1) { [string]$p.ProxyServer }").ok()
}
