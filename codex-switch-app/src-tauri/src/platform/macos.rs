use super::{home_dir, AppTarget};
use std::{
    env,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

pub(super) fn data_dir() -> PathBuf {
    home_dir().join("Documents/CodexSwitch")
}

pub(super) fn find_app() -> Option<AppTarget> {
    let candidates =
        if let Some(explicit) = env::var_os("CODEX_SWITCH_APP_PATH").filter(|v| !v.is_empty()) {
            vec![PathBuf::from(explicit)]
        } else {
            vec![
                PathBuf::from("/Applications/Codex.app"),
                home_dir().join("Applications/Codex.app"),
                PathBuf::from("/Applications/ChatGPT.app"),
                home_dir().join("Applications/ChatGPT.app"),
            ]
        };
    candidates.into_iter().find_map(|path| {
        let path = path.canonicalize().ok()?;
        if path.extension()?.to_str()? != "app" || !path.join("Contents/Info.plist").is_file() {
            return None;
        }
        let name = path.file_stem()?.to_string_lossy().into_owned();
        Some(AppTarget {
            name,
            target: path.to_string_lossy().into_owned(),
        })
    })
}

fn target_processes() -> Result<Vec<u32>, String> {
    let Some(app) = find_app() else {
        return Ok(Vec::new());
    };
    let prefix = format!("{}/Contents/", app.target);
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,comm="])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("Unable to inspect Codex processes.".into());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (pid, executable) = line.trim().split_once(char::is_whitespace)?;
            let pid = pid.parse::<u32>().ok()?;
            (pid != std::process::id() && executable.trim().starts_with(&prefix)).then_some(pid)
        })
        .collect())
}

pub(super) fn app_is_running() -> Result<bool, String> {
    target_processes().map(|pids| !pids.is_empty())
}

pub(super) fn stop_app() -> Result<String, String> {
    for pid in target_processes()? {
        let _ = Command::new("/bin/kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while app_is_running()? && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }
    // Re-read executable identities before escalating, rather than retaining stale PIDs.
    for pid in target_processes()? {
        let _ = Command::new("/bin/kill")
            .args(["-KILL", &pid.to_string()])
            .status();
    }
    thread::sleep(Duration::from_millis(100));
    if app_is_running()? {
        return Err("Codex is still running; login files were not changed.".into());
    }
    Ok("Codex stopped. Main process count after: 0.".into())
}

pub(super) fn start_app(app: &AppTarget) -> Result<String, String> {
    let status = Command::new("/usr/bin/open")
        .arg("-a")
        .arg(&app.target)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("Failed to launch {}.", app.target));
    }
    Ok(format!("App started: {} -> {}.", app.name, app.target))
}

pub(super) fn open_path(path: &Path) -> Result<(), String> {
    let status = Command::new("/usr/bin/open")
        .arg(path)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Failed to open {}.", path.display()))
    }
}
pub(super) fn open_url(url: &str) -> Result<(), String> {
    open_path(Path::new(url))
}

pub(super) fn cli_command() -> Command {
    let mut search: Vec<PathBuf> = env::var_os("PATH")
        .map(|v| env::split_paths(&v).collect())
        .unwrap_or_default();
    search.extend([
        home_dir().join(".local/bin"),
        home_dir().join(".cargo/bin"),
        home_dir().join("Library/pnpm"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ]);
    let executable = env::var_os("CODEX_SWITCH_CODEX_CLI")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            search
                .iter()
                .map(|dir| dir.join("codex"))
                .find(|path| path.is_file())
        })
        .unwrap_or_else(|| PathBuf::from("codex"));
    let mut command = Command::new(executable);
    if let Ok(path) = env::join_paths(search) {
        command.env("PATH", path);
    }
    // Each managed CLI gets its own group so cancellation cannot kill this app.
    command.process_group(0);
    command
}

pub(super) fn terminate_process(process_id: u32) {
    if process_id <= 1 || process_id == std::process::id() {
        return;
    }
    let group = format!("-{process_id}");
    let _ = Command::new("/bin/kill").args(["-TERM", &group]).status();
    thread::sleep(Duration::from_millis(100));
    let _ = Command::new("/bin/kill").args(["-KILL", &group]).status();
}

pub(super) fn system_proxy() -> Option<String> {
    let output = Command::new("/usr/sbin/scutil")
        .arg("--proxy")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_proxy(&String::from_utf8_lossy(&output.stdout))
}

fn parse_proxy(raw: &str) -> Option<String> {
    let value = |key: &str| {
        raw.lines().find_map(|line| {
            let (name, value) = line.trim().split_once(" : ")?;
            (name == key).then_some(value.trim())
        })
    };
    for prefix in ["HTTPS", "HTTP"] {
        if value(&format!("{prefix}Enable")) != Some("1") {
            continue;
        }
        let host = value(&format!("{prefix}Proxy"))?;
        let port = value(&format!("{prefix}Port"))?.parse::<u16>().ok()?;
        let host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.to_string()
        };
        return Some(format!("http://{host}:{port}"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_proxy_uses_enabled_https_before_http() {
        assert_eq!(
            parse_proxy("HTTPSEnable : 1\nHTTPSProxy : 127.0.0.1\nHTTPSPort : 7890").as_deref(),
            Some("http://127.0.0.1:7890")
        );
        assert!(parse_proxy("HTTPEnable : 0\nHTTPProxy : 127.0.0.1\nHTTPPort : 7890").is_none());
    }
}
