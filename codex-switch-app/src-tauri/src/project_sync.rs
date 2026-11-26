use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const STATE_FILE: &str = ".codex-global-state.json";
const ACCOUNT_SCOPES: &str = "electron-account-renderer-state-v1";
const PROJECTS: &str = "local-projects";
const ORDER: &str = "project-order";
const MAPPINGS: &str = "app-server-project-id-by-legacy-project-id-by-host";
const ASSIGNMENTS: &str = "thread-project-assignments";
const MEMBERSHIP_HOSTS: &str = "thread-project-membership-host-ids";
const PROJECTLESS: &str = "projectless-thread-ids";
const MIGRATIONS: &str = "app-server-projects-migration-by-host";
const LEGACY_ROOTS: [&str; 2] = ["electron-saved-workspace-roots", "active-workspace-roots"];

/// The account-scoped project and local thread bindings carried through a switch.
#[derive(Clone, Deserialize, Serialize)]
pub(super) struct ProjectSync {
    codex_dir: PathBuf,
    source_owner: String,
    target_owner: String,
    projects: Map<String, Value>,
    order: Option<Vec<Value>>,
    local_mappings: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    thread_assignments: Map<String, Value>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ProjectSyncResult {
    pub(super) projects: usize,
    pub(super) threads: usize,
}

pub(super) fn renderer_owner(user_id: &str, account_id: &str) -> String {
    // Codex Desktop keys renderer state by the SHA-256 of JSON.stringify([userId, accountId]).
    let identity = json!([user_id, account_id]).to_string();
    format!("{:x}", Sha256::digest(identity.as_bytes()))
}

fn parse_state(raw: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(raw.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(raw))
        .map_err(|error| format!("Unable to read Codex project state: {error}"))
}

fn optional_object<'a>(
    value: &'a Value,
    key: &str,
) -> Result<Option<&'a Map<String, Value>>, String> {
    value
        .get(key)
        .filter(|field| !field.is_null())
        .map(|field| {
            field
                .as_object()
                .ok_or_else(|| format!("Unsupported Codex project state field: {key}."))
        })
        .transpose()
}

fn optional_array<'a>(value: &'a Value, key: &str) -> Result<Option<&'a Vec<Value>>, String> {
    value
        .get(key)
        .filter(|field| !field.is_null())
        .map(|field| {
            field
                .as_array()
                .ok_or_else(|| format!("Unsupported Codex project state field: {key}."))
        })
        .transpose()
}

fn scope_owner(state: &Value) -> Result<Option<&str>, String> {
    let scope = &state[ACCOUNT_SCOPES];
    if scope["inactive"].as_object().is_none() {
        return Err("Codex account-scoped project state is unavailable.".to_string());
    }
    match scope.get("owner") {
        Some(Value::Null) => Ok(None),
        Some(Value::String(owner)) if !owner.is_empty() => Ok(Some(owner)),
        _ => Err("Codex project state has no recognizable renderer owner.".to_string()),
    }
}

fn local_host(codex_dir: &Path) -> String {
    format!("local:{}", codex_dir.display())
}

fn local_project_binding(id: &str, project: &Value) -> Option<Value> {
    // Cloud-backed records can include a source account or user identity.
    let cloud_fields = [
        "chatGptBacking",
        "chatGptBackingCreation",
        "retiredChatGptBacking",
        "chatGptBackingRecoveryPending",
    ];
    if id.starts_with("g-p-") || cloud_fields.iter().any(|key| !project[*key].is_null()) {
        return None;
    }
    let root_paths = project["rootPaths"].as_array()?;
    if project["id"].as_str() != Some(id)
        || !root_paths.iter().all(Value::is_string)
        || !project["createdAt"].is_number()
        || !project["updatedAt"].is_number()
    {
        return None;
    }
    Some(json!({
        "id": id,
        "name": project["name"].as_str()?,
        "rootPaths": root_paths,
        "createdAt": project["createdAt"],
        "updatedAt": project["updatedAt"]
    }))
}

pub(super) fn capture(
    codex_dir: &Path,
    source_owner: &str,
    target_owner: &str,
) -> Result<Option<ProjectSync>, String> {
    if source_owner == target_owner {
        return Ok(None);
    }
    let raw = match fs::read(codex_dir.join(STATE_FILE)) {
        Ok(raw) => raw,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Unable to read Codex project state: {error}")),
    };
    let state = parse_state(&raw)?;
    let mut snapshot = capture_state(&state, codex_dir, source_owner, target_owner)?;
    if let Some(snapshot) = &mut snapshot {
        recover_relocated_threads(&state, snapshot);
    }
    Ok(snapshot)
}

fn account_state<'a>(state: &'a Value, owner: &str) -> Option<&'a Value> {
    if scope_owner(state).ok()? == Some(owner) {
        Some(state)
    } else {
        state[ACCOUNT_SCOPES]["inactive"][owner].get("globals")
    }
}

fn normalized_root(root: &str) -> String {
    crate::platform::normalize_project_root(root)
}

fn recover_relocated_threads(state: &Value, snapshot: &mut ProjectSync) {
    let Some(source) = account_state(state, &snapshot.source_owner) else {
        return;
    };
    let Some(writable_roots) = source["thread-writable-roots"].as_object() else {
        return;
    };
    let Ok(entries) = fs::read_dir(&snapshot.codex_dir) else {
        return;
    };
    let mut backups: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            if !(name.starts_with(".codex-global-state.json.before-project-restore-")
                || name.starts_with(".codex-global-state.json.before-project-sync-"))
                || !name.ends_with(".bak")
            {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            (metadata.is_file() && metadata.len() <= 32 * 1024 * 1024)
                .then(|| (metadata.modified().ok(), entry.path()))
        })
        .collect();
    backups.sort_unstable_by(|a, b| b.cmp(a));
    let current_roots: HashSet<_> = snapshot
        .projects
        .values()
        .flat_map(|project| project["rootPaths"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .map(normalized_root)
        .collect();
    let mut old_roots = HashMap::new();
    for (_, backup) in backups.into_iter().take(16) {
        let Ok(old_state) =
            fs::read(backup).and_then(|raw| parse_state(&raw).map_err(io::Error::other))
        else {
            continue;
        };
        let Some(old_source) = account_state(&old_state, &snapshot.source_owner) else {
            continue;
        };
        for id in snapshot.projects.keys() {
            let host = local_host(&snapshot.codex_dir);
            if let Some(server_id) = snapshot.local_mappings.get(id) {
                if old_source[MAPPINGS][&host][id] != *server_id {
                    continue;
                }
            }
            let Some(old_project) = local_project_binding(id, &old_source[PROJECTS][id]) else {
                continue;
            };
            for old_root in old_project["rootPaths"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                let root = normalized_root(old_root);
                // Accept an exact old root from the same account and stable project ID.
                // Any ambiguity across projects leaves the thread unassigned.
                if root.len() <= 3 || current_roots.contains(&root) {
                    continue;
                }
                let binding = old_roots.entry(root).or_insert_with(|| Some(id.clone()));
                if binding.as_ref() != Some(id) {
                    *binding = None;
                }
            }
        }
    }
    if old_roots.is_empty() {
        return;
    }
    let Ok(index) = File::open(snapshot.codex_dir.join("session_index.jsonl")) else {
        return;
    };
    let indexed_threads: HashSet<_> = BufReader::new(index)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .filter_map(|entry| entry["id"].as_str().map(str::to_owned))
        .collect();
    for (id, roots) in writable_roots {
        if !indexed_threads.contains(id)
            || !source[ASSIGNMENTS][id].is_null()
            || source[PROJECTLESS]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|value| value == id))
            || !source[MEMBERSHIP_HOSTS][id].is_null() && source[MEMBERSHIP_HOSTS][id] != "local"
        {
            continue;
        }
        let Some(root) = roots.get(0).and_then(Value::as_str) else {
            continue;
        };
        if let Some(Some(project_id)) = old_roots.get(&normalized_root(root)) {
            snapshot.thread_assignments.insert(
                id.clone(),
                json!({"projectKind": "local", "projectId": project_id}),
            );
        }
    }
}

fn capture_state(
    state: &Value,
    codex_dir: &Path,
    source_owner: &str,
    target_owner: &str,
) -> Result<Option<ProjectSync>, String> {
    if state.get(ACCOUNT_SCOPES).is_none() {
        return Ok(None);
    }
    let source = match scope_owner(state)? {
        Some(owner) if owner == source_owner => state,
        Some(_) => {
            return Err("The signed-in profile does not match the Codex project owner.".to_string())
        }
        None => &state[ACCOUNT_SCOPES]["inactive"][source_owner]["globals"],
    };
    let projects: Map<_, _> = optional_object(source, PROJECTS)?
        .into_iter()
        .flatten()
        .filter_map(|(id, project)| {
            local_project_binding(id, project).map(|binding| (id.clone(), binding))
        })
        .collect();
    if projects.is_empty() {
        return Ok(None);
    }
    let order = optional_array(source, ORDER)?.map(|order| {
        order
            .iter()
            .filter(|id| id.as_str().is_some_and(|id| projects.contains_key(id)))
            .cloned()
            .collect()
    });
    optional_object(source, MAPPINGS)?;
    let local_mappings = optional_object(&source[MAPPINGS], &local_host(codex_dir))?
        .into_iter()
        .flatten()
        .filter(|(id, server_id)| {
            projects.contains_key(*id) && server_id.as_str().is_some_and(|id| !id.is_empty())
        })
        .map(|(id, server_id)| (id.clone(), server_id.clone()))
        .collect();
    let source_hosts = optional_object(source, MEMBERSHIP_HOSTS)?;
    let source_projectless = optional_array(source, PROJECTLESS)?;
    let thread_assignments = optional_object(source, ASSIGNMENTS)?
        .into_iter()
        .flatten()
        .filter_map(|(thread_id, assignment)| {
            let project_id = assignment["projectId"].as_str()?;
            let host = source_hosts.and_then(|hosts| hosts.get(thread_id));
            if assignment["projectKind"] != "local"
                || !assignment["projectOrigin"].is_null()
                || !assignment["hostId"].is_null() && assignment["hostId"] != "local"
                || host.is_some_and(|host| !host.is_null() && host != "local")
                || source_projectless.is_some_and(|ids| ids.iter().any(|id| id == thread_id))
                || !projects.contains_key(project_id)
            {
                return None;
            }
            Some((
                thread_id.clone(),
                json!({ "projectId": project_id, "projectKind": "local" }),
            ))
        })
        .collect();

    Ok(Some(ProjectSync {
        codex_dir: codex_dir.to_path_buf(),
        source_owner: source_owner.to_string(),
        target_owner: target_owner.to_string(),
        projects,
        order,
        local_mappings,
        thread_assignments,
    }))
}

fn matching_target_projects(
    snapshot: &ProjectSync,
    target: &Value,
) -> Result<Map<String, Value>, String> {
    let Some(target_projects) = optional_object(target, PROJECTS)? else {
        return Ok(Map::new());
    };
    let native_targets: Vec<_> = target_projects
        .iter()
        .filter(|(id, project)| local_project_binding(id, project).is_some())
        .map(|(id, _)| id.as_str())
        .collect();
    let host = local_host(&snapshot.codex_dir);
    optional_object(target, MAPPINGS)?;
    let target_mappings = optional_object(&target[MAPPINGS], &host)?;
    let target_server_id = |id: &str| target_mappings?.get(id)?.as_str();

    let mut matches = Map::new();
    for source_id in snapshot.projects.keys() {
        let source_server_id = snapshot
            .local_mappings
            .get(source_id)
            .and_then(Value::as_str);
        if native_targets.contains(&source_id.as_str())
            && !matches!((source_server_id, target_server_id(source_id)), (Some(a), Some(b)) if a != b)
        {
            matches.insert(source_id.clone(), json!(source_id));
            continue;
        }
        // Desktop persists memberships by server project ID in a database shared by accounts.
        // A same-folder project with a different server ID must not gain these memberships.
        let Some(source_server_id) = source_server_id else {
            continue;
        };
        let mut aliases = native_targets
            .iter()
            .filter(|id| target_server_id(id) == Some(source_server_id));
        if let (Some(id), None) = (aliases.next(), aliases.next()) {
            matches.insert(source_id.clone(), json!(id));
        }
    }
    Ok(matches)
}

fn restore_thread_assignments(target: &mut Value, snapshot: &ProjectSync) -> Result<usize, String> {
    if snapshot.thread_assignments.is_empty() {
        return Ok(0);
    }
    let project_matches = matching_target_projects(snapshot, target)?;
    let mut assignments = optional_object(target, ASSIGNMENTS)?
        .cloned()
        .unwrap_or_default();
    let mut hosts = optional_object(target, MEMBERSHIP_HOSTS)?
        .cloned()
        .unwrap_or_default();
    let projectless = optional_array(target, PROJECTLESS)?;
    let mut added = Vec::new();
    for (thread_id, assignment) in &snapshot.thread_assignments {
        if assignments.contains_key(thread_id)
            || projectless.is_some_and(|ids| ids.iter().any(|id| id == thread_id))
            || hosts
                .get(thread_id)
                .is_some_and(|host| !host.is_null() && host != "local")
        {
            continue;
        }
        let Some(project_id) = assignment["projectId"]
            .as_str()
            .and_then(|id| project_matches.get(id))
        else {
            continue;
        };
        assignments.insert(
            thread_id.clone(),
            json!({ "projectKind": "local", "projectId": project_id }),
        );
        hosts.insert(thread_id.clone(), json!("local"));
        added.push(json!(thread_id));
    }
    if added.is_empty() {
        return Ok(0);
    }

    let host = local_host(&snapshot.codex_dir);
    let mut migrations = optional_object(target, MIGRATIONS)?
        .cloned()
        .unwrap_or_default();
    // Fresh scopes already run the native migration. For a scope that ran it, queue just
    // these bindings so Desktop can update its local database through the app-server.
    let update_migration = migrations.get_mut(&host).filter(|state| !state.is_null());
    if let Some(migration) = update_migration {
        if !migration.is_object() {
            return Err("The target Codex project migration state is invalid.".to_string());
        }
        let mut pending = optional_array(migration, "pendingThreadAssignmentIds")?
            .cloned()
            .unwrap_or_default();
        for id in &added {
            if !pending.contains(id) {
                pending.push(id.clone());
            }
        }
        migration["pendingThreadAssignmentIds"] = Value::Array(pending);
        migration["threadAssignmentsMigrated"] = Value::Bool(false);
        target[MIGRATIONS] = Value::Object(migrations);
    }
    target[ASSIGNMENTS] = Value::Object(assignments);
    target[MEMBERSHIP_HOSTS] = Value::Object(hosts);
    Ok(added.len())
}

fn restore_state(
    state: &mut Value,
    snapshot: &ProjectSync,
    target_owner: &str,
) -> Result<Option<ProjectSyncResult>, String> {
    if snapshot.target_owner != target_owner || snapshot.source_owner == target_owner {
        return Err(
            "The target profile does not match the pending Codex project sync.".to_string(),
        );
    }
    let active = match scope_owner(state)? {
        Some(owner) if owner == target_owner => true,
        Some(owner) if owner != snapshot.source_owner => {
            return Err("The Codex project owner changed during the profile switch.".to_string());
        }
        _ => false,
    };

    // Preserve the live renderer owner. Desktop selects the target's stored scope on login.
    let target = if active {
        state
    } else {
        let inactive = state[ACCOUNT_SCOPES]["inactive"]
            .as_object_mut()
            .ok_or("The Codex account project scopes are invalid.")?;
        let profile = inactive
            .entry(target_owner)
            .or_insert_with(|| json!({ "atoms": {}, "globals": {} }));
        profile
            .get_mut("globals")
            .filter(|globals| globals.is_object())
            .ok_or("The target Codex project scope is invalid.")?
    };

    let mut populated =
        optional_object(target, PROJECTS)?.is_some_and(|projects| !projects.is_empty());
    for key in LEGACY_ROOTS {
        populated |= optional_array(target, key)?.is_some_and(|roots| !roots.is_empty());
    }
    let projects = if populated || snapshot.projects.is_empty() {
        0
    } else {
        restore_project_definitions(target, snapshot)?;
        snapshot.projects.len()
    };
    let threads = restore_thread_assignments(target, snapshot)?;
    Ok((projects > 0 || threads > 0).then_some(ProjectSyncResult { projects, threads }))
}

fn restore_project_definitions(target: &mut Value, snapshot: &ProjectSync) -> Result<(), String> {
    let order = snapshot
        .order
        .as_ref()
        .map(|source_order| {
            let mut target_order = optional_array(target, ORDER)?.cloned().unwrap_or_default();
            target_order.retain(|id| {
                !id.as_str()
                    .is_some_and(|id| snapshot.projects.contains_key(id))
            });
            target_order.extend(source_order.iter().cloned());
            Ok::<_, String>(target_order)
        })
        .transpose()?;

    if !snapshot.local_mappings.is_empty() {
        let mut mappings = optional_object(target, MAPPINGS)?
            .cloned()
            .unwrap_or_default();
        let host = mappings
            .entry(local_host(&snapshot.codex_dir))
            .or_insert_with(|| json!({}));
        let local = host
            .as_object_mut()
            .ok_or("The target Codex project mappings are invalid.")?;
        for (id, server_id) in &snapshot.local_mappings {
            if local.get(id).is_some_and(|existing| existing != server_id) {
                return Err(
                    "The target profile already has a different Codex project ID binding."
                        .to_string(),
                );
            }
            local.insert(id.clone(), server_id.clone());
        }
        target[MAPPINGS] = Value::Object(mappings);
    }
    target[PROJECTS] = Value::Object(snapshot.projects.clone());
    if let Some(order) = order {
        target[ORDER] = Value::Array(order);
    }
    Ok(())
}

pub(super) fn restore(
    codex_dir: &Path,
    snapshot: &ProjectSync,
    target_owner: &str,
) -> Result<Option<ProjectSyncResult>, String> {
    if codex_dir != snapshot.codex_dir {
        return Err("Codex home changed during the profile switch.".to_string());
    }
    let path = codex_dir.join(STATE_FILE);
    let raw =
        fs::read(&path).map_err(|error| format!("Unable to read Codex project state: {error}"))?;
    let mut state = parse_state(&raw)?;
    let result = restore_state(&mut state, snapshot, target_owner)?;
    if result.is_some() {
        let updated = serde_json::to_vec(&state).map_err(|error| error.to_string())?;
        write_state(&path, &raw, &updated)
            .map_err(|error| format!("Unable to update Codex project state: {error}"))?;
    }
    Ok(result)
}

fn write_state(path: &Path, before: &[u8], after: &[u8]) -> io::Result<()> {
    let id = Uuid::new_v4().simple().to_string();
    let temporary = path.with_file_name(format!(".codex-project-sync-{id}.tmp"));
    let backup = path.with_file_name(format!("{STATE_FILE}.before-project-sync-{id}.bak"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(after)?;
        file.sync_all()?;
        drop(file);
        if fs::read(path)? != before {
            return Err(io::Error::other(
                "Codex project state changed while preparing the update.",
            ));
        }
        replace_with_backup(path, &temporary, &backup)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(windows)]
fn replace_with_backup(path: &Path, replacement: &Path, backup: &Path) -> io::Result<()> {
    use std::{ffi::c_void, os::windows::ffi::OsStrExt, ptr::null_mut};

    #[link(name = "kernel32")]
    extern "system" {
        fn ReplaceFileW(
            replaced: *const u16,
            replacement: *const u16,
            backup: *const u16,
            flags: u32,
            exclude: *mut c_void,
            reserved: *mut c_void,
        ) -> i32;
    }

    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    let (path, replacement, backup) = (wide(path), wide(replacement), wide(backup));
    // All three paths remain in the same directory. ReplaceFileW also preserves the file ACL.
    let result = unsafe {
        ReplaceFileW(
            path.as_ptr(),
            replacement.as_ptr(),
            backup.as_ptr(),
            0,
            null_mut(),
            null_mut(),
        )
    };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_with_backup(path: &Path, replacement: &Path, backup: &Path) -> io::Result<()> {
    fs::copy(path, backup)?;
    fs::rename(replacement, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "source";
    const TARGET: &str = "target";

    fn fixture(codex_dir: &Path) -> Value {
        json!({
            ACCOUNT_SCOPES: {"owner": SOURCE, "inactive": {
                TARGET: {"atoms": {"keep": true}, "globals": {"other-setting": 23}},
                "third": {"atoms": {}, "globals": {PROJECTS: {"other": {}}}}
            }},
            PROJECTS: {
                "a": {"id": "a", "name": "A", "rootPaths": [r"C:\src\a"], "createdAt": 1, "updatedAt": 2},
                "b": {"id": "b", "name": "B", "rootPaths": [r"C:\src\b"], "createdAt": 3, "updatedAt": 4}
            },
            ORDER: ["b", "remote", "a"],
            MAPPINGS: {
                (local_host(codex_dir)): {"a": "server-a", "b": "server-b", "stale": "server-c"},
                "ssh:another": {"a": "remote-a"}
            },
            ASSIGNMENTS: {"thread": {"projectKind": "local", "projectId": "a"}},
            "other-global": "unchanged"
        })
    }

    #[test]
    fn captures_native_project_bindings_without_cloud_links_or_unknown_fields() {
        let home = Path::new("codex");
        let mut state = fixture(home);
        state[PROJECTS]["a"]["unknown"] = json!(123);
        state[PROJECTS]["b"]["chatGptBackingCreation"] = json!({"accountId": "source"});
        let mut mirror = state[PROJECTS]["a"].clone();
        mirror["id"] = json!("g-p-cloud");
        state[PROJECTS]["g-p-cloud"] = mirror;
        state[ORDER] = json!(["g-p-cloud", "b", "a"]);
        state[ASSIGNMENTS]["cloud-project"] = json!({"projectKind": "local", "projectId": "b"});
        state[ASSIGNMENTS]["cloud-thread"] =
            json!({"projectKind": "local", "projectId": "a", "projectOrigin": "chatgpt"});
        state[ASSIGNMENTS]["remote-thread"] = json!({"projectKind": "local", "projectId": "a"});
        state[MEMBERSHIP_HOSTS] = json!({"remote-thread": "remote"});
        let snapshot = capture_state(&state, home, SOURCE, TARGET)
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.projects.len(), 1);
        assert!(snapshot.projects["a"].get("unknown").is_none());
        assert_eq!(snapshot.order, Some(vec![json!("a")]));
        assert_eq!(snapshot.local_mappings.len(), 1);
        assert_eq!(snapshot.local_mappings["a"], "server-a");
        assert_eq!(
            snapshot.thread_assignments,
            state[ASSIGNMENTS]
                .as_object()
                .unwrap()
                .iter()
                .filter(|(id, _)| *id == "thread")
                .map(|(id, v)| (id.clone(), v.clone()))
                .collect()
        );
    }

    #[test]
    fn restores_project_and_local_thread_bindings_to_an_empty_target_scope() {
        let home = Path::new("codex");
        let mut state = fixture(home);
        let original = state.clone();
        let snapshot = capture_state(&state, home, SOURCE, TARGET)
            .unwrap()
            .unwrap();
        assert_eq!(
            restore_state(&mut state, &snapshot, TARGET).unwrap(),
            Some(ProjectSyncResult {
                projects: 2,
                threads: 1
            })
        );
        let target = &state[ACCOUNT_SCOPES]["inactive"][TARGET];
        assert_eq!(target["globals"][PROJECTS], original[PROJECTS]);
        assert_eq!(target["globals"][ORDER], json!(["b", "a"]));
        assert_eq!(
            target["globals"][MAPPINGS],
            json!({
                (local_host(home)): {"a": "server-a", "b": "server-b"}
            })
        );
        assert_eq!(target["globals"]["other-setting"], 23);
        assert_eq!(target["globals"][ASSIGNMENTS], original[ASSIGNMENTS]);
        assert_eq!(
            target["globals"][MEMBERSHIP_HOSTS],
            json!({"thread": "local"})
        );
        assert_eq!(target["atoms"], json!({"keep": true}));
        assert_eq!(state[PROJECTS], original[PROJECTS]);
        assert_eq!(state[ACCOUNT_SCOPES]["owner"], SOURCE);
        assert_eq!(
            state[ACCOUNT_SCOPES]["inactive"]["third"],
            original[ACCOUNT_SCOPES]["inactive"]["third"]
        );
        let restored = state.clone();
        assert_eq!(restore_state(&mut state, &snapshot, TARGET).unwrap(), None);
        assert_eq!(state, restored);
    }

    #[test]
    fn fills_missing_memberships_in_existing_projects_without_rebinding_other_threads() {
        let home = Path::new("codex");
        let mut state = fixture(home);
        let a = json!({"projectKind": "local", "projectId": "a"});
        let b = json!({"projectKind": "local", "projectId": "b"});
        state[ASSIGNMENTS] = json!({
            "thread": a, "already-assigned": a, "projectless": a, "other-server-project": b
        });
        let snapshot = capture_state(&state, home, SOURCE, TARGET)
            .unwrap()
            .unwrap();
        let mut renamed = state[PROJECTS]["a"].clone();
        renamed["id"] = json!("alias");
        let target = &mut state[ACCOUNT_SCOPES]["inactive"][TARGET]["globals"];
        target[PROJECTS] = json!({"alias": renamed, "b": snapshot.projects["b"]});
        target[ORDER] = json!(["alias", "b"]);
        target[MAPPINGS] = json!({(local_host(home)): {"alias": "server-a", "b": "different-b"}});
        target[ASSIGNMENTS] =
            json!({"already-assigned": {"projectKind": "remote", "projectId": "remote"}});
        target[PROJECTLESS] = json!(["projectless"]);
        target[MIGRATIONS] = json!({
            (local_host(home)): {"projectsMigrated": true, "threadAssignmentsMigrated": true,
                "threadAssignmentsReadMigrated": true, "pendingThreadAssignmentIds": ["pending"]},
            "remote": {"projectsMigrated": false},
        });
        let original = target.clone();
        assert_eq!(
            restore_state(&mut state, &snapshot, TARGET).unwrap(),
            Some(ProjectSyncResult {
                projects: 0,
                threads: 1
            })
        );
        let target = &state[ACCOUNT_SCOPES]["inactive"][TARGET]["globals"];
        assert_eq!(target[PROJECTS], original[PROJECTS]);
        assert_eq!(target[ORDER], original[ORDER]);
        assert_eq!(target[MAPPINGS], original[MAPPINGS]);
        assert_eq!(target[PROJECTLESS], original[PROJECTLESS]);
        assert_eq!(
            target[ASSIGNMENTS],
            json!({
                "already-assigned": original[ASSIGNMENTS]["already-assigned"],
                "thread": {"projectKind": "local", "projectId": "alias"}
            })
        );
        assert_eq!(
            target[MIGRATIONS][local_host(home)]["pendingThreadAssignmentIds"],
            json!(["pending", "thread"])
        );
        assert_eq!(
            target[MIGRATIONS][local_host(home)]["threadAssignmentsMigrated"],
            false
        );
        assert_eq!(target[MIGRATIONS]["remote"], original[MIGRATIONS]["remote"]);
    }

    #[test]
    fn prior_switch_journal_has_no_thread_assignments() {
        let home = Path::new("codex");
        let mut state = fixture(home);
        let snapshot = capture_state(&state, home, SOURCE, TARGET)
            .unwrap()
            .unwrap();
        let mut previous = serde_json::to_value(snapshot).unwrap();
        previous
            .as_object_mut()
            .unwrap()
            .remove("thread_assignments");
        let previous = serde_json::from_value::<ProjectSync>(previous).unwrap();
        assert_eq!(
            restore_state(&mut state, &previous, TARGET).unwrap(),
            Some(ProjectSyncResult {
                projects: 2,
                threads: 0
            })
        );
    }

    #[test]
    fn preserves_existing_target_projects_and_legacy_roots() {
        let home = Path::new("codex");
        let mut state = fixture(home);
        let snapshot = capture_state(&state, home, SOURCE, TARGET)
            .unwrap()
            .unwrap();
        for (key, value) in [
            (PROJECTS, json!({"existing": {}})),
            (LEGACY_ROOTS[0], json!(["folder"])),
        ] {
            let globals = &mut state[ACCOUNT_SCOPES]["inactive"][TARGET]["globals"];
            *globals = json!({ (key): value });
            let original = state.clone();
            assert_eq!(restore_state(&mut state, &snapshot, TARGET).unwrap(), None);
            assert_eq!(state, original);
        }
    }

    #[test]
    fn targets_new_and_active_scopes_without_using_another_account() {
        let home = Path::new("codex");
        let mut state = fixture(home);
        let snapshot = capture_state(&state, home, SOURCE, TARGET)
            .unwrap()
            .unwrap();
        assert!(restore_state(&mut state, &snapshot, "third").is_err());
        assert!(capture_state(&state, home, "third", TARGET).is_err());
        state[ACCOUNT_SCOPES]["inactive"]
            .as_object_mut()
            .unwrap()
            .remove(TARGET);
        assert_eq!(
            restore_state(&mut state, &snapshot, TARGET).unwrap(),
            Some(ProjectSyncResult {
                projects: 2,
                threads: 1
            })
        );
        assert_eq!(
            state[ACCOUNT_SCOPES]["inactive"][TARGET]["atoms"],
            json!({})
        );
        state[ACCOUNT_SCOPES]["owner"] = json!(TARGET);
        let active = state.as_object_mut().unwrap();
        active.remove(PROJECTS);
        active.remove(ASSIGNMENTS);
        assert_eq!(
            restore_state(&mut state, &snapshot, TARGET).unwrap(),
            Some(ProjectSyncResult {
                projects: 2,
                threads: 1
            })
        );
        assert!(state[PROJECTS].is_object());
    }
}
