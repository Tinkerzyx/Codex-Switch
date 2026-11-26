//! Operating-system integration. Business code uses this module, never shell names.
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

pub mod floating;
#[cfg(target_os = "macos")]
mod macos;
pub mod web_session;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
use macos as native;
#[cfg(windows)]
use windows as native;
#[cfg(not(any(windows, target_os = "macos")))]
compile_error!("Codex Switch currently supports Windows and macOS desktop targets.");

#[derive(Clone, Debug)]
pub(crate) struct AppTarget {
    pub name: String,
    pub target: String,
}

pub(crate) fn home_dir() -> PathBuf {
    dirs::home_dir().expect("Codex Switch requires a user home directory")
}

fn path_override(key: &str) -> Option<PathBuf> {
    env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub(crate) fn codex_dir() -> PathBuf {
    path_override("CODEX_HOME").unwrap_or_else(|| home_dir().join(".codex"))
}

pub(crate) fn data_dir() -> PathBuf {
    path_override("CODEX_SWITCH_DATA_DIR").unwrap_or_else(native::data_dir)
}

pub(crate) fn find_app() -> Option<AppTarget> {
    native::find_app()
}
pub(crate) fn stop_app() -> Result<String, String> {
    native::stop_app()
}
pub(crate) fn app_is_running() -> Result<bool, String> {
    native::app_is_running()
}
pub(crate) fn start_app(app: &AppTarget) -> Result<String, String> {
    native::start_app(app)
}
pub(crate) fn open_path(path: &Path) -> Result<(), String> {
    native::open_path(path)
}
pub(crate) fn open_url(url: &str) -> Result<(), String> {
    native::open_url(url)
}
pub(crate) fn cli_command() -> Command {
    native::cli_command()
}
pub(crate) fn terminate_process(process_id: u32) {
    native::terminate_process(process_id)
}
pub(crate) fn system_proxy() -> Option<String> {
    native::system_proxy()
}
pub(crate) fn capture_payment_rect(
    window: &tauri::WebviewWindow,
    value: &serde_json::Value,
) -> Result<String, String> {
    #[cfg(windows)]
    {
        windows::capture::capture_rect(window, value)
    }
    #[cfg(target_os = "macos")]
    {
        let _ = (window, value);
        Err("This payment page requires native QR capture. Open the payment link in your browser to continue.".into())
    }
}

/// Windows uses case-insensitive paths. Unix paths retain case and separators.
pub(crate) fn normalize_project_root(root: &str) -> String {
    normalize_root_for_platform(root, cfg!(windows))
}

fn normalize_root_for_platform(root: &str, windows: bool) -> String {
    if windows {
        let root = root.replace('/', "\\");
        root.strip_prefix(r"\\?\")
            .unwrap_or(&root)
            .trim_end_matches('\\')
            .to_lowercase()
    } else if root == "/" {
        root.to_string()
    } else {
        root.trim_end_matches('/').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_paths_follow_platform_case_and_separator_rules() {
        assert_eq!(
            normalize_root_for_platform(r"\\?\C:\Work\Project\", true),
            r"c:\work\project"
        );
        assert_eq!(
            normalize_root_for_platform("C:/Work/Project/", true),
            r"c:\work\project"
        );
        assert_ne!(
            normalize_root_for_platform("/Users/me/Project", false),
            normalize_root_for_platform("/Users/me/project", false)
        );
        assert_eq!(
            normalize_root_for_platform("/Users/me/项目/", false),
            "/Users/me/项目"
        );
        assert_eq!(normalize_root_for_platform("/", false), "/");
        assert_eq!(
            normalize_root_for_platform(r"/Users/me/a\b", false),
            r"/Users/me/a\b"
        );
    }
}
