//! Sidecar processes: semantic analyzers written in the language's own toolchain (Roslyn for
//! C#, the TypeScript compiler API for TypeScript) that speak line-delimited JSON.
//!
//! Protocol: one JSON object per line on stdin, one per line on stdout.
//! - request: `{"id": 1, "method": "open", "params": {"root": "D:/project"}}`
//! - response: `{"id": 1, "result": {...}}` or `{"id": 1, "error": "message"}`
//!
//! Lines that are not JSON, or responses to other ids, are ignored, so a sidecar may log to
//! stdout by accident without breaking the stream.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::OnceLock;

use nodex_ir::semantic::SemanticFacts;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// The id of a response line.
#[derive(Deserialize)]
struct Head {
    id: Option<u64>,
}

/// A response line: a result or an error.
#[derive(Deserialize)]
struct Response<T> {
    result: Option<T>,
    error: Option<String>,
}

/// A running sidecar process.
pub struct LineSidecar {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    /// Display name used in error messages (`Roslyn`, `TypeScript`).
    name: &'static str,
}

impl LineSidecar {
    /// Starts `command` with piped stdin/stdout and no console window. Stderr is inherited when
    /// `log` is true; otherwise its lines go to the Nodex log.
    pub fn spawn(mut command: Command, name: &'static str, log: bool) -> Result<Self, String> {
        crate::process::hide_window(&mut command);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(if log { Stdio::inherit() } else { Stdio::piped() })
            .spawn()
            .map_err(|e| format!("cannot start the {name} sidecar ({e})"))?;
        if let Some(stderr) = child.stderr.take() {
            let _ = std::thread::Builder::new().name(format!("nodex-{name}-stderr")).spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if !line.trim().is_empty() {
                        tracing::info!(target: "nodex_sidecar", sidecar = name, "{line}");
                    }
                }
            });
        }
        let stdin = child.stdin.take().ok_or("sidecar stdin unavailable")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("sidecar stdout unavailable")?);
        Ok(Self { child, stdin, stdout, next_id: 1, name })
    }

    /// Sends a request and waits for its response.
    pub fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.request_as(method, params)
    }

    /// Sends a request whose result is a [`SemanticFacts`] object.
    pub fn facts(&mut self, method: &str, params: Value) -> Result<SemanticFacts, String> {
        self.request_as(method, params)
    }

    /// Sends a request and reads its result straight into `T` (the facts of a large project are
    /// tens of megabytes on one line: no intermediate JSON tree).
    fn request_as<T: DeserializeOwned>(&mut self, method: &str, params: Value) -> Result<T, String> {
        let name = self.name;
        let id = self.next_id;
        self.next_id += 1;
        let line = json!({ "id": id, "method": method, "params": params }).to_string();
        writeln!(self.stdin, "{line}")
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("{name} sidecar stopped ({e})"))?;
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = self.stdout.read_line(&mut buf).map_err(|e| format!("{name} sidecar read failed ({e})"))?;
            if read == 0 {
                return Err(format!("{name} sidecar exited unexpectedly"));
            }
            // Only the id first (other fields are skipped, not built), then the whole response.
            let Ok(head) = serde_json::from_str::<Head>(buf.trim()) else { continue };
            if head.id != Some(id) {
                continue;
            }
            let response: Response<T> =
                serde_json::from_str(buf.trim()).map_err(|e| format!("unexpected response from the {name} sidecar: {e}"))?;
            if let Some(err) = response.error {
                return Err(format!("{name}: {err}"));
            }
            return match response.result {
                Some(result) => Ok(result),
                // `null` (or no result) for a method that returns nothing.
                None => serde_json::from_value(Value::Null).map_err(|e| format!("unexpected response from the {name} sidecar: {e}")),
            };
        }
    }
}

impl Drop for LineSidecar {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "{}", json!({ "id": 0, "method": "shutdown" }));
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The installed app's resource folder, set once by the desktop host at start-up.
static RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Tells the locators where the installed app keeps its resources (the Tauri resource folder,
/// which is not the executable's folder on every system). Later calls are ignored.
pub fn set_resource_dir(dir: impl Into<PathBuf>) {
    let _ = RESOURCE_DIR.set(plain_path(dir.into()));
}

/// `C:\x` for a Windows verbatim path `\\?\C:\x` (what Tauri reports), which some tools passed
/// such a path as an argument do not accept; UNC (`\\?\UNC\…`) and other paths are kept.
fn plain_path(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_drive_paths_become_plain() {
        assert_eq!(plain_path(PathBuf::from(r"\\?\C:\Apps\Nodex")), PathBuf::from(r"C:\Apps\Nodex"));
        assert_eq!(plain_path(PathBuf::from(r"\\?\UNC\server\share")), PathBuf::from(r"\\?\UNC\server\share"));
        assert_eq!(plain_path(PathBuf::from("/opt/nodex")), PathBuf::from("/opt/nodex"));
    }
}

/// Folders an installed Nodex keeps its sidecars in: the resource folder, then the
/// executable's folder.
pub fn install_dirs() -> Vec<PathBuf> {
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    RESOURCE_DIR.get().cloned().into_iter().chain(exe_dir).collect()
}

/// Folders whose ancestors may hold a source checkout: the executable's folder and, in debug
/// builds only, this crate (so a release binary never carries the build machine's paths).
fn source_roots() -> Vec<PathBuf> {
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    #[allow(unused_mut)]
    let mut roots: Vec<PathBuf> = exe_dir.into_iter().collect();
    // An attribute, not `cfg!`: the path must not even be compiled into a release build.
    #[cfg(debug_assertions)]
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    roots
}

/// Finds a sidecar entry file: the `env` variable, `installed` relative to the resource or
/// executable folder, or one of `source_tree` relative to any ancestor of the executable (or, in
/// debug builds, of this crate: a build in the repository).
pub fn locate(env: &str, installed: &[&str], source_tree: &[&str]) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(env).map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    for dir in install_dirs() {
        for rel in installed {
            let candidate = dir.join(rel);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    for start in source_roots() {
        for dir in start.ancestors() {
            for rel in source_tree {
                let candidate = dir.join(rel);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}
