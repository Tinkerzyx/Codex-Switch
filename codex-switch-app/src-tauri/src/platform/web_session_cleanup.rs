//! Shared session lifecycle with native Windows and macOS cache backends.
#[cfg(target_os = "macos")]
#[path = "macos_web_session_cleanup.rs"]
mod macos;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

static SESSIONS: Mutex<BTreeMap<String, Usage>> = Mutex::new(BTreeMap::new());
const RELEASE_GRACE: Duration = Duration::from_secs(60);
#[cfg(not(target_os = "macos"))]
const CACHE_PATHS: &[&str] = &[
    "EBWebView/Default/Cache",
    "EBWebView/Default/Code Cache",
    "EBWebView/Default/GPUCache",
    "EBWebView/Default/DawnCache",
    "EBWebView/Default/DawnGraphiteCache",
    "EBWebView/Default/DawnWebGPUCache",
    "EBWebView/Default/Service Worker/CacheStorage",
    "EBWebView/GPUCache",
    "EBWebView/DawnCache",
    "EBWebView/GrShaderCache",
    "EBWebView/GraphiteDawnCache",
    "EBWebView/GPUPersistentCache",
    "EBWebView/ShaderCache",
];

struct Usage {
    windows: usize,
    released: Option<Instant>,
    maintaining: bool,
}

impl Usage {
    fn busy(&self) -> bool {
        self.windows > 0
            || self.maintaining
            || self
                .released
                .is_some_and(|released| released.elapsed() < RELEASE_GRACE)
    }
}

pub(crate) struct SessionLease(String);

pub(crate) fn acquire(id: &str) -> Result<SessionLease, String> {
    if !valid_id(id) {
        return Err("Invalid managed web session ID.".into());
    }
    let mut sessions = SESSIONS
        .lock()
        .map_err(|_| "Web session lock unavailable.")?;
    let usage = sessions.entry(id.to_ascii_lowercase()).or_insert(Usage {
        windows: 0,
        released: None,
        maintaining: false,
    });
    if usage.maintaining {
        return Err("The web session is being maintained. Please retry shortly.".into());
    }
    usage.windows += 1;
    Ok(SessionLease(id.to_ascii_lowercase()))
}

impl SessionLease {
    pub(crate) fn attach(self, window: &tauri::WebviewWindow) {
        let lease = Mutex::new(Some(self));
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                if let Ok(mut lease) = lease.lock() {
                    lease.take();
                }
            }
        });
    }
}

impl Drop for SessionLease {
    fn drop(&mut self) {
        if let Ok(mut sessions) = SESSIONS.lock() {
            if let Some(usage) = sessions.get_mut(&self.0) {
                usage.windows = usage.windows.saturating_sub(1);
                usage.released = Some(Instant::now());
            }
        }
    }
}

struct MaintenanceLease(String);

fn reserve_maintenance(id: &str) -> Result<Option<MaintenanceLease>, String> {
    let mut sessions = SESSIONS
        .lock()
        .map_err(|_| "Web session lock unavailable.")?;
    let usage = sessions.entry(id.to_string()).or_insert(Usage {
        windows: 0,
        released: None,
        maintaining: false,
    });
    if usage.busy() {
        return Ok(None);
    }
    usage.maintaining = true;
    Ok(Some(MaintenanceLease(id.to_string())))
}

impl Drop for MaintenanceLease {
    fn drop(&mut self) {
        if let Ok(mut sessions) = SESSIONS.lock() {
            if let Some(usage) = sessions.get_mut(&self.0) {
                usage.maintaining = false;
            }
        }
    }
}

fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CleanupReport {
    pub removed_sessions: usize,
    pub removed_cache_directories: usize,
    pub removed_files: usize,
    pub freed_bytes: u64,
    pub skipped_busy_sessions: usize,
    pub skipped_unsafe_entries: usize,
    pub failed_entries: usize,
    pub cleared_webkit_caches: usize,
}

fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

// Check every path component before descending; never traverse junctions or links.
fn safe_directory(root: &Path, relative: &Path) -> io::Result<Option<PathBuf>> {
    let mut path = root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(io::Error::other("Unsafe cleanup path"));
        }
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() && !linked(&metadata) => {}
            Ok(_) => return Err(io::Error::other("Linked or non-directory cleanup path")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
    }
    Ok(Some(path))
}

fn remove_tree(path: &Path, report: &mut CleanupReport) -> bool {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return true,
        Err(_) => {
            report.failed_entries += 1;
            return false;
        }
    };
    if linked(&metadata) {
        report.skipped_unsafe_entries += 1;
        return false;
    }
    if metadata.is_dir() {
        let entries = match fs::read_dir(path) {
            Ok(entries) => entries,
            Err(_) => {
                report.failed_entries += 1;
                return false;
            }
        };
        let mut complete = true;
        for entry in entries {
            match entry {
                Ok(entry) => complete &= remove_tree(&entry.path(), report),
                Err(_) => {
                    report.failed_entries += 1;
                    complete = false;
                }
            }
        }
        if !complete {
            return false;
        }
        if fs::remove_dir(path).is_ok() {
            true
        } else {
            report.failed_entries += 1;
            false
        }
    } else if fs::remove_file(path).is_ok() {
        report.removed_files += 1;
        report.freed_bytes += metadata.len();
        true
    } else {
        report.failed_entries += 1;
        false
    }
}

/// Reference lookup happens under the same lock used to reserve new browser windows.
pub(crate) fn cleanup(
    app: Option<&tauri::AppHandle>,
    root: &Path,
    references: impl FnOnce() -> Result<(BTreeSet<String>, BTreeSet<String>), String>,
) -> Result<CleanupReport, String> {
    let sessions = SESSIONS
        .lock()
        .map_err(|_| "Web session lock unavailable.")?;
    let mut report = CleanupReport::default();
    #[cfg(target_os = "macos")]
    if app.is_none() {
        return Err("WebKit maintenance requires a running app event loop.".into());
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(report),
        Err(error) => return Err(error.to_string()),
    };
    if linked(&metadata) || !metadata.is_dir() {
        return Err("Web session root is linked or is not a directory; cleanup skipped.".into());
    }
    let (retained, protected) = references()?;
    // Never hold this lock while waiting for a main-thread WebKit completion.
    drop(sessions);
    let retained: BTreeSet<_> = retained
        .into_iter()
        .map(|id| id.to_ascii_lowercase())
        .collect();
    let protected: BTreeSet<_> = protected
        .into_iter()
        .map(|id| id.to_ascii_lowercase())
        .collect();
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    for entry in fs::read_dir(&root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let id = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !valid_id(&id) {
            report.skipped_unsafe_entries += 1;
            continue;
        }
        if protected.contains(&id) {
            report.skipped_busy_sessions += 1;
            continue;
        }
        let path = match safe_directory(&root, Path::new(&id)) {
            Ok(Some(path)) => path,
            Ok(None) => continue,
            Err(_) => {
                report.skipped_unsafe_entries += 1;
                continue;
            }
        };
        let Some(maintenance) = reserve_maintenance(&id)? else {
            report.skipped_busy_sessions += 1;
            continue;
        };
        #[cfg(target_os = "macos")]
        {
            let retained = retained.contains(&id);
            match macos::clean(app.unwrap(), &id, !retained, maintenance) {
                Ok(()) if retained => report.cleared_webkit_caches += 1,
                Ok(()) => {
                    // Keep the marker if native removal fails, so the next sweep can retry.
                    if remove_tree(&path, &mut report) {
                        report.removed_sessions += 1;
                    }
                }
                Err(_) => report.failed_entries += 1,
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _maintenance = maintenance;
            if !retained.contains(&id) {
                if remove_tree(&path, &mut report) {
                    report.removed_sessions += 1;
                }
            } else {
                for cache in CACHE_PATHS {
                    match safe_directory(&path, Path::new(cache)) {
                        Ok(Some(cache)) => {
                            if remove_tree(&cache, &mut report) {
                                report.removed_cache_directories += 1;
                            }
                        }
                        Ok(None) => {}
                        Err(_) => report.skipped_unsafe_entries += 1,
                    }
                }
            }
        }
    }
    SESSIONS
        .lock()
        .map_err(|_| "Web session lock unavailable.")?
        .retain(|_, usage| usage.busy());
    Ok(report)
}

#[cfg(test)]
mod lease_tests {
    use super::*;

    #[test]
    fn native_maintenance_excludes_new_windows_until_completion() {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let reservation = reserve_maintenance(&id).unwrap().unwrap();
        assert!(acquire(&id).is_err());
        assert!(reserve_maintenance(&id).unwrap().is_none());
        drop(reservation);
        let window = acquire(&id).unwrap();
        assert!(reserve_maintenance(&id).unwrap().is_none());
        drop(window);
        assert!(reserve_maintenance(&id).unwrap().is_none());
        SESSIONS.lock().unwrap().get_mut(&id).unwrap().released =
            Some(Instant::now() - RELEASE_GRACE);
        assert!(reserve_maintenance(&id).unwrap().is_some());
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("csw-cleanup-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn write(&self, relative: &str) {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"keep-login-data").unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn retains_credentials_removes_only_allowed_caches_and_orphans() {
        let f = Fixture::new();
        let keep = "a".repeat(32);
        let orphan = "b".repeat(32);
        for name in [
            "Network/Cookies",
            "Local Storage/leveldb/data",
            "IndexedDB/data",
            "History",
            "Service Worker/Database/data",
            "UnknownCache/data",
        ] {
            f.write(&format!("{keep}/EBWebView/Default/{name}"));
        }
        f.write(&format!("{keep}/EBWebView/Default/Cache/Cache_Data/data"));
        f.write(&format!("{keep}/EBWebView/GrShaderCache/data"));
        f.write(&format!("{orphan}/EBWebView/Default/Network/Cookies"));
        f.write("unexpected/data");
        let report = cleanup(None, &f.0, || Ok(([keep.clone()].into(), BTreeSet::new()))).unwrap();
        assert_eq!(report.removed_sessions, 1);
        assert_eq!(report.removed_cache_directories, 2);
        assert_eq!(report.failed_entries, 0);
        assert!(f
            .0
            .join(format!("{keep}/EBWebView/Default/Network/Cookies"))
            .exists());
        assert!(f
            .0
            .join(format!(
                "{keep}/EBWebView/Default/Local Storage/leveldb/data"
            ))
            .exists());
        assert!(f
            .0
            .join(format!("{keep}/EBWebView/Default/IndexedDB/data"))
            .exists());
        assert!(f
            .0
            .join(format!(
                "{keep}/EBWebView/Default/Service Worker/Database/data"
            ))
            .exists());
        assert!(f
            .0
            .join(format!("{keep}/EBWebView/Default/UnknownCache/data"))
            .exists());
        assert!(f.0.join("unexpected/data").exists());
        assert!(!f.0.join(orphan).exists());
    }

    #[test]
    fn protects_pending_login_active_windows_and_release_grace() {
        let f = Fixture::new();
        let pending = "c".repeat(32);
        let active = "d".repeat(32);
        f.write(&format!("{pending}/data"));
        f.write(&format!("{active}/data"));
        let lease = acquire(&active).unwrap();
        let references = || Ok((BTreeSet::new(), [pending.clone()].into()));
        assert_eq!(
            cleanup(None, &f.0, references)
                .unwrap()
                .skipped_busy_sessions,
            2
        );
        drop(lease);
        assert_eq!(
            cleanup(None, &f.0, references)
                .unwrap()
                .skipped_busy_sessions,
            2
        );
        assert!(f.0.join(active).exists());
        assert!(f.0.join(pending).exists());
    }

    #[test]
    fn unreadable_references_fail_closed_and_invalid_ids_are_rejected() {
        let f = Fixture::new();
        let id = "e".repeat(32);
        f.write(&format!("{id}/data"));
        assert!(cleanup(None, &f.0, || Err("invalid profiles.json".into())).is_err());
        assert!(f.0.join(id).exists());
        assert!(acquire("../outside").is_err());
    }

    #[test]
    fn locked_files_are_deferred_and_removed_on_the_next_pass() {
        use std::os::windows::fs::OpenOptionsExt;
        let f = Fixture::new();
        let id = "1".repeat(32);
        f.write(&format!("{id}/data"));
        let file = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(f.0.join(format!("{id}/data")))
            .unwrap();
        let references = || Ok((BTreeSet::new(), BTreeSet::new()));
        let report = cleanup(None, &f.0, references).unwrap();
        assert_eq!(report.failed_entries, 1);
        assert_eq!(report.removed_sessions, 0);
        drop(file);
        assert_eq!(cleanup(None, &f.0, references).unwrap().removed_sessions, 1);
        assert!(!f.0.join(id).exists());
    }

    #[test]
    fn skips_junctions_without_touching_their_targets() {
        let f = Fixture::new();
        let outside = Fixture::new();
        outside.write("data");
        let id = "f".repeat(32);
        let junction = f.0.join(&id);
        let status = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside.0)
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(
            cleanup(None, &f.0, || Ok((BTreeSet::new(), BTreeSet::new())))
                .unwrap()
                .skipped_unsafe_entries,
            1
        );
        assert!(outside.0.join("data").exists());
        fs::remove_dir(junction).unwrap();

        let default = f.0.join(&id).join("EBWebView").join("Default");
        fs::create_dir_all(&default).unwrap();
        let cache_link = default.join("Cache");
        let status = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&cache_link)
            .arg(&outside.0)
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(
            cleanup(None, &f.0, || Ok(([id].into(), BTreeSet::new())))
                .unwrap()
                .skipped_unsafe_entries,
            1
        );
        assert!(outside.0.join("data").exists());
        fs::remove_dir(cache_link).unwrap();
    }
}
