//! Debug adapters: where they are, and installing them when the user asks.
//!
//! Each language names a pinned adapter build ([`nodex_lang_api::AdapterSpec`]). Nothing is
//! downloaded until the user clicks *Install*; the archive is checked against its pinned SHA-256
//! before it is unpacked into the per-user cache folder (`%LOCALAPPDATA%\<app id>\adapters`,
//! `~/Library/Caches/nodex/adapters`, `$XDG_CACHE_HOME/nodex/adapters`; `NODEX_ADAPTERS_DIR`
//! overrides it). Each adapter's own environment variable (`NODEX_NETCOREDBG`, …) can point to
//! one installed elsewhere.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nodex_ir::Language;
use nodex_lang_api::{AdapterSpec, LanguageFrontend};
use serde::Serialize;
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::{AppState, ServerEvent};

/// A language's debug adapter, as the UI shows it.
#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct AdapterStatus {
    pub language: Language,
    pub name: String,
    pub version: String,
    pub homepage: String,
    /// `installed`, `missing`, `override` (from an environment variable), `unsupported` (no
    /// build for this machine).
    pub state: String,
    pub path: Option<String>,
    /// Download size in bytes.
    #[ts(type = "number")]
    pub size: u64,
    pub url: Option<String>,
}

/// Download progress of an adapter.
#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct AdapterProgress {
    pub language: Language,
    #[ts(type = "number")]
    pub received: u64,
    #[ts(type = "number")]
    pub total: u64,
    pub done: bool,
    pub error: Option<String>,
}

pub fn cache_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("NODEX_ADAPTERS_DIR") {
        return PathBuf::from(d);
    }
    crate::projects::data_dir().join("adapters")
}

fn install_dir(spec: &AdapterSpec) -> PathBuf {
    cache_dir().join(format!("{}-{}", spec.name, spec.version))
}

/// Finds `name` anywhere below `dir` (archives sometimes nest one more folder).
fn find_file(dir: &Path, name: &str, depth: usize) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            subdirs.push(p);
        } else if p.file_name().is_some_and(|n| n == name) {
            return Some(p);
        }
    }
    if depth == 0 {
        return None;
    }
    subdirs.into_iter().find_map(|d| find_file(&d, name, depth - 1))
}

/// The adapter's program, if it is installed (or overridden): `(path, from_override)`.
pub fn locate(spec: &AdapterSpec) -> Option<(PathBuf, bool)> {
    if let Some(p) = std::env::var_os(spec.env_override).map(PathBuf::from)
        && p.exists()
    {
        return Some((p, true));
    }
    let download = spec.download()?;
    let dir = install_dir(spec);
    let exact = dir.join(download.program);
    if exact.is_file() {
        return Some((exact, false));
    }
    let name = Path::new(download.program).file_name()?.to_string_lossy().into_owned();
    find_file(&dir, &name, 4).map(|p| (p, false))
}

pub fn status(language: Language, spec: &AdapterSpec) -> AdapterStatus {
    let download = spec.download();
    let (state, path) = match (locate(spec), download) {
        (Some((p, true)), _) => ("override", Some(p)),
        (Some((p, false)), _) => ("installed", Some(p)),
        (None, Some(_)) => ("missing", None),
        (None, None) => ("unsupported", None),
    };
    AdapterStatus {
        language,
        name: spec.name.to_owned(),
        version: spec.version.to_owned(),
        homepage: spec.homepage.to_owned(),
        state: state.to_owned(),
        path: path.map(|p| p.to_string_lossy().into_owned()),
        size: download.map_or(0, |d| d.size),
        url: download.map(|d| d.url.to_owned()),
    }
}

/// Adapters of the project's languages (for the project at `root`: a Unity project needs
/// another adapter than a .NET program).
pub fn statuses(frontends: &[Arc<dyn LanguageFrontend>], root: &Path) -> Vec<AdapterStatus> {
    frontends.iter().filter_map(|f| f.debugger(root).map(|d| status(f.id(), &d.adapter()))).collect()
}

/// Downloads, verifies and unpacks an adapter, reporting progress through `progress`.
pub fn install(spec: &AdapterSpec, progress: &mut dyn FnMut(u64, u64)) -> Result<PathBuf, String> {
    let download = spec.download().ok_or("this adapter has no build for this machine")?;
    let dir = install_dir(spec);
    let root = cache_dir();
    std::fs::create_dir_all(&root).map_err(|e| format!("cannot create {}: {e}", root.display()))?;
    let archive = root.join(format!("{}-{}.download", spec.name, spec.version));

    // Download while hashing.
    let mut response = ureq::get(download.url).call().map_err(|e| format!("download failed: {e}"))?;
    let total = response.body().content_length().unwrap_or(download.size);
    let mut reader = response.body_mut().as_reader();
    let mut file = std::fs::File::create(&archive).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut received = 0u64;
    let mut last = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("download failed: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        received += n as u64;
        if received - last > 256 * 1024 {
            progress(received, total);
            last = received;
        }
    }
    drop(file);
    progress(received, total);
    let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if digest != download.sha256 {
        let _ = std::fs::remove_file(&archive);
        return Err(format!("the download does not match its published checksum (got {digest}); nothing was installed"));
    }

    // Unpack next to the final folder, then swap it in.
    let staging = root.join(format!("{}-{}.unpacking", spec.name, spec.version));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let file = std::fs::File::open(&archive).map_err(|e| e.to_string())?;
    match download.format {
        "zip" => {
            let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("bad archive: {e}"))?;
            zip.extract(&staging).map_err(|e| format!("cannot unpack: {e}"))?;
        }
        "tar.gz" => {
            let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
            tar.unpack(&staging).map_err(|e| format!("cannot unpack: {e}"))?;
        }
        other => return Err(format!("unknown archive format {other}")),
    }
    let _ = std::fs::remove_file(&archive);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&staging, &dir).map_err(|e| format!("cannot move the adapter into place: {e}"))?;
    let (program, _) = locate(spec).ok_or("the archive did not contain the adapter")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755));
    }
    Ok(program)
}

/// Adapters being downloaded now (by name).
static INSTALLING: std::sync::LazyLock<Mutex<std::collections::HashSet<&'static str>>> = std::sync::LazyLock::new(Default::default);

/// Installs the adapter of `language` in the background; browsers follow `adapter-progress`.
pub fn start_install(state: &Arc<AppState>, language: Language) -> Result<(), String> {
    let frontend = state.frontends.iter().find(|f| f.id() == language).ok_or("unknown language")?;
    let root = state.project.lock().map_err(|_| "project lock poisoned")?.root().to_path_buf();
    let spec = frontend.debugger(&root).ok_or("this language has no debugger")?.adapter();
    // One install per adapter at a time: two would write the same archive and staging folder.
    if !INSTALLING.lock().map_err(|_| "poisoned")?.insert(spec.name) {
        return Err(format!("{} is already being installed", spec.name));
    }
    let name = spec.name;
    let state = state.clone();
    std::thread::Builder::new()
        .name("nodex-adapter-install".into())
        .spawn(move || {
            let send = |received: u64, total: u64, done: bool, error: Option<String>| {
                let _ = state.events.send(ServerEvent::AdapterProgress(AdapterProgress { language, received, total, done, error }));
            };
            let result = install(&spec, &mut |r, t| send(r, t, false, None));
            if let Ok(mut busy) = INSTALLING.lock() {
                busy.remove(spec.name);
            }
            match result {
                Ok(path) => {
                    tracing::info!(adapter = spec.name, path = %path.display(), "debug adapter installed");
                    send(1, 1, true, None);
                }
                Err(e) => {
                    tracing::warn!(adapter = spec.name, "adapter install failed: {e}");
                    send(0, 0, true, Some(e));
                }
            }
        })
        .map_err(|e| {
            if let Ok(mut busy) = INSTALLING.lock() {
                busy.remove(name);
            }
            e.to_string()
        })?;
    Ok(())
}
