//! Opening another project while the server runs, browsing folders, and the recent list.
//!
//! Switching stops what belongs to the old project (a run, the file watcher, semantic workers),
//! then loads the new one. Semantic workers of the old project may still be busy: they check
//! [`AppState::generation`] and drop their results.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use nodex_core::Project;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{AppState, ServerEvent, semantic_worker, watch};

/// A folder in the open dialog.
#[derive(Serialize, TS, Clone, Debug, PartialEq, Eq)]
#[ts(export)]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
    /// Project kinds found by a quick look at the folder: `csharp`, `unity`, `typescript`, `rust`.
    pub kinds: Vec<String>,
}

/// One listed folder and its subfolders.
#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct FolderListing {
    /// The listed folder; empty for the top level (drives and the home folder).
    pub path: String,
    pub parent: Option<String>,
    /// Kinds of the listed folder itself.
    pub kinds: Vec<String>,
    pub entries: Vec<FolderEntry>,
    pub home: Option<String>,
}

/// A project opened before.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct RecentProject {
    pub path: String,
    pub name: String,
    pub languages: Vec<String>,
    /// Seconds since the Unix epoch.
    pub opened_at: u64,
}

const SKIP: &[&str] = &["node_modules", "target", "bin", "obj", "Library", "Temp", "dist", "build", "__pycache__"];
const MAX_RECENT: usize = 12;

/// What kinds of project a folder looks like, from the files directly in it.
pub fn folder_kinds(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut kinds = Vec::new();
    let mut add = |k: &str| {
        if !kinds.iter().any(|x| x == k) {
            kinds.push(k.to_owned());
        }
    };
    let (mut assets, mut settings) = (false, false);
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_lowercase();
        let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
        match name.as_str() {
            "assets" if is_dir => assets = true,
            "projectsettings" if is_dir => settings = true,
            "package.json" | "tsconfig.json" | "jsconfig.json" => add("typescript"),
            "cargo.toml" => add("rust"),
            n if !is_dir && (n.ends_with(".csproj") || n.ends_with(".sln") || n.ends_with(".slnx")) => add("csharp"),
            _ => {}
        }
    }
    if assets && settings {
        add("unity");
    }
    kinds
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}

fn display(p: &Path) -> String {
    let s = p.to_string_lossy().into_owned();
    // `\\?\D:\x` from canonicalize on Windows.
    s.strip_prefix(r"\\?\").map(str::to_owned).unwrap_or(s)
}

/// Lists the subfolders of `path`; an empty path lists the drives (Windows) or `/`, plus home.
pub fn list(path: &str) -> Result<FolderListing, String> {
    let home = home_dir().map(|h| display(&h));
    if path.trim().is_empty() {
        let mut entries = Vec::new();
        if cfg!(windows) {
            for letter in b'A'..=b'Z' {
                let drive = format!("{}:\\", letter as char);
                if Path::new(&drive).is_dir() {
                    entries.push(FolderEntry { name: drive.clone(), path: drive, kinds: Vec::new() });
                }
            }
        } else {
            entries.push(FolderEntry { name: "/".into(), path: "/".into(), kinds: Vec::new() });
        }
        return Ok(FolderListing { path: String::new(), parent: None, kinds: Vec::new(), entries, home });
    }
    let dir = PathBuf::from(path.trim());
    let read = std::fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut entries: Vec<FolderEntry> = read
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.starts_with('$') || SKIP.contains(&name.as_str()) {
                return None;
            }
            let p = e.path();
            Some(FolderEntry { kinds: folder_kinds(&p), path: display(&p), name })
        })
        .collect();
    entries.sort_by_key(|e| e.name.to_lowercase());
    let parent = dir.parent().map(display).filter(|p| !p.is_empty());
    Ok(FolderListing { path: display(&dir), parent: parent.or(Some(String::new())), kinds: folder_kinds(&dir), entries, home })
}

/// The app's identifier (`identifier` in `crates/nodex-desktop/tauri.conf.json`). On Windows it
/// names Nodex's folders under `%APPDATA%` and `%LOCALAPPDATA%`: the installer puts the program in
/// `%LOCALAPPDATA%\Nodex`, so data must live elsewhere, and its "Delete the application data"
/// option removes these folders.
pub const APP_ID: &str = "io.github.legustavinho.nodex";

/// Where Nodex keeps per-user settings: `%APPDATA%\<APP_ID>`, `~/Library/Application Support/nodex`,
/// `$XDG_CONFIG_HOME/nodex` or `~/.config/nodex`. `NODEX_CONFIG_DIR` overrides it.
pub fn config_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("NODEX_CONFIG_DIR") {
        return Some(PathBuf::from(d));
    }
    if cfg!(windows) {
        return std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(APP_ID));
    }
    if cfg!(target_os = "macos") {
        return home_dir().map(|h| h.join("Library/Application Support/nodex"));
    }
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| home_dir().map(|h| h.join(".config"))).map(|c| c.join("nodex"))
}

/// Where Nodex keeps downloads, logs and other data that can be rebuilt: `%LOCALAPPDATA%\<APP_ID>`,
/// `~/Library/Caches/nodex`, `$XDG_CACHE_HOME/nodex` or `~/.cache/nodex` (the temp folder if none).
pub fn data_dir() -> PathBuf {
    if cfg!(windows) {
        return std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join(APP_ID);
    }
    let base = if cfg!(target_os = "macos") {
        home_dir().map(|h| h.join("Library/Caches"))
    } else {
        std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or_else(|| home_dir().map(|h| h.join(".cache")))
    };
    base.unwrap_or_else(std::env::temp_dir).join("nodex")
}

fn recent_file() -> Option<PathBuf> {
    let file = config_dir().map(|d| d.join("recent.json"))?;
    // Earlier builds kept the list in %APPDATA%\nodex: bring it over once.
    if cfg!(windows)
        && std::env::var_os("NODEX_CONFIG_DIR").is_none()
        && !file.exists()
        && let Some(old) = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("nodex").join("recent.json"))
        && old.is_file()
        && let Some(dir) = file.parent()
    {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::copy(old, &file);
    }
    Some(file)
}

pub fn recent() -> Vec<RecentProject> {
    recent_file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|t| serde_json::from_str::<Vec<RecentProject>>(&t).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|r| Path::new(&r.path).is_dir())
        .collect()
}

/// Puts the project first in the recent list.
pub fn remember(project: &Project) {
    let Some(file) = recent_file() else { return };
    let summary = project.summary();
    let entry = RecentProject {
        path: display(project.root()),
        name: summary.name,
        languages: summary.languages.iter().map(|l| format!("{l:?}").to_lowercase()).collect(),
        opened_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
    };
    let mut list: Vec<RecentProject> = recent().into_iter().filter(|r| !same_path(&r.path, &entry.path)).collect();
    list.insert(0, entry);
    list.truncate(MAX_RECENT);
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(&list) {
        let _ = std::fs::write(file, text);
    }
}

fn same_path(a: &str, b: &str) -> bool {
    if cfg!(windows) { a.eq_ignore_ascii_case(b) } else { a == b }
}

/// Opens the project at `path` in place of the current one.
pub async fn open(state: &Arc<AppState>, path: String) -> Result<(), String> {
    let dir = PathBuf::from(path.trim());
    if !dir.is_dir() {
        return Err(format!("{} is not a folder", dir.display()));
    }
    let _ = state.events.send(ServerEvent::ProjectLoading { path: display(&dir) });
    state.run.stop();
    let _ = state.debug.command(crate::debugger::DebugCommand::Stop);
    let frontends = state.frontends.clone();
    let timer = nodex_core::perf::phase("open.total");
    let loaded = tokio::task::spawn_blocking(move || Project::open(&dir, frontends))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("cannot open the project: {e}"));
    let project = match loaded {
        Ok(p) => p,
        Err(e) => {
            let _ = state.events.send(ServerEvent::ProjectFailed { error: e.clone() });
            return Err(e);
        }
    };
    tracing::info!(root = %project.root().display(), files = project.summary().files.len(), "project opened");
    remember(&project);
    let installing = nodex_core::perf::phase("open.install");
    install(state, project)?;
    installing.finish("");
    timer.finish("(until project-changed)");
    let _ = state.events.send(ServerEvent::ProjectChanged);
    Ok(())
}

/// Makes `project` the current one and restarts what watches it.
pub fn install(state: &Arc<AppState>, project: Project) -> Result<(), String> {
    state.generation.fetch_add(1, Ordering::SeqCst);
    // Old semantic workers exit when their channel closes.
    if let Ok(mut senders) = state.semantic_tx.lock() {
        senders.clear();
    }
    // The old watcher stops when dropped; its thread ends with its channel.
    if let Ok(mut w) = state.watcher.lock() {
        *w = None;
    }
    state.run.reset();
    let open = project.is_open();
    *state.project.lock().map_err(|_| "project lock poisoned".to_owned())? = project;
    if !open {
        return Ok(());
    }
    let watcher = watch::spawn(state.clone()).map_err(|e| format!("cannot watch the project folder: {e}"))?;
    if let Ok(mut w) = state.watcher.lock() {
        *w = Some(watcher);
    }
    semantic_worker::spawn(state);
    Ok(())
}

/// Closes the folder: back to the welcome screen (an empty project).
pub fn close(state: &Arc<AppState>) -> Result<(), String> {
    state.run.stop();
    let _ = state.debug.command(crate::debugger::DebugCommand::Stop);
    install(state, Project::empty(state.frontends.clone()))?;
    let _ = state.events.send(ServerEvent::ProjectChanged);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_id_matches_the_desktop_identifier() {
        let conf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../nodex-desktop/tauri.conf.json");
        let conf: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(conf).unwrap()).unwrap();
        assert_eq!(conf["identifier"], APP_ID);
    }

    #[test]
    fn kinds_of_the_fixtures() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        assert_eq!(folder_kinds(&fixtures.join("rust/Sample")), ["rust"]);
        assert_eq!(folder_kinds(&fixtures.join("ts/Sample")), ["typescript"]);
        assert!(folder_kinds(&fixtures.join("csharp/Sample")).contains(&"csharp".to_owned()));
        let listing = list(&fixtures.to_string_lossy()).unwrap();
        assert!(listing.entries.iter().any(|e| e.name == "rust"));
        let top = list("").unwrap();
        assert!(!top.entries.is_empty());
    }
}
