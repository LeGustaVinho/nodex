//! Unity projects run in the Editor the user opened, not in a process Nodex starts:
//! - the Nodex package (`sidecars/unity/com.nodex.trace`) compiles trace probes into the
//!   project's scripts while `Library/Nodex/trace-on` exists, writes the trace maps to
//!   `Library/Nodex/maps/`, and its Editor bridge does what `.nodex/unity/session.json` asks
//!   (connect to the trace intake in Play Mode, press Play, switch to Debug mode);
//! - live traces are an *attached* run: Nodex opens its trace intake and waits for the Editor;
//! - debugging attaches unity-dap (Mono soft debugger) to the Editor's debugger port.

use std::path::{Path, PathBuf};

use nodex_lang_api::{
    AdapterDownload, AdapterSpec, AdapterTransport, DebugLaunch, DebugLauncher, RunCommand, RunEnv, RunKind, TraceInstrumenter,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const PACKAGE: &str = "com.nodex.trace";

// ---------------------------------------------------------------------------------------------
// The Editor

/// `Library/EditorInstance.json`, written by the Editor while it has the project open.
#[derive(Deserialize, Debug, Clone)]
pub struct EditorInstance {
    pub process_id: u32,
    #[serde(default)]
    pub version: String,
}

/// `.nodex/unity/editor.json`, written by the Nodex bridge.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct BridgeState {
    #[serde(default)]
    pub package: String,
    #[serde(default)]
    pub unity: String,
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub playing: bool,
    #[serde(default)]
    pub compiling: bool,
    #[serde(default, rename = "codeOptimization")]
    pub code_optimization: String,
    #[serde(default, rename = "traceCompiled")]
    pub trace_compiled: bool,
    #[serde(default, rename = "traceRequested")]
    pub trace_requested: bool,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub updated: i64,
}

pub fn is_unity(root: &Path) -> bool {
    root.join("ProjectSettings").is_dir() || (root.join("Assets").is_dir() && root.join("Packages").join("manifest.json").is_file())
}

/// The Editor that has the project open, if its process is alive.
pub fn editor(root: &Path) -> Option<EditorInstance> {
    let text = std::fs::read_to_string(root.join("Library").join("EditorInstance.json")).ok()?;
    let instance: EditorInstance = serde_json::from_str(&text).ok()?;
    process_alive(instance.process_id).then_some(instance)
}

fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        nodex_lang_api::process::command("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("\"{pid}\"")))
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        nodex_lang_api::process::command("kill").args(["-0", &pid.to_string()]).status().map(|s| s.success()).unwrap_or(false)
    }
}

/// The bridge's last report, when it is recent (the bridge rewrites it every 2 s).
pub fn bridge(root: &Path) -> Option<BridgeState> {
    let text = std::fs::read_to_string(root.join(".nodex").join("unity").join("editor.json")).ok()?;
    let state: BridgeState = serde_json::from_str(&text).ok()?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    (now - state.updated <= 6).then_some(state)
}

/// The Mono soft debugger port of the Editor with this process id.
pub fn debugger_port(pid: u32) -> u16 {
    56000 + (pid % 1000) as u16
}

/// What Nodex asks of the Editor (`.nodex/unity/session.json`; the bridge reads it).
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Session {
    pub id: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub play: bool,
    #[serde(default)]
    pub debug: bool,
}

fn session_file(root: &Path) -> PathBuf {
    root.join(".nodex").join("unity").join("session.json")
}

pub fn read_session(root: &Path) -> Option<Session> {
    serde_json::from_str(&std::fs::read_to_string(session_file(root)).ok()?).ok()
}

/// Writes the session (atomically: the bridge polls it), or removes it for `None`.
pub fn write_session(root: &Path, session: Option<&Session>) -> Result<(), String> {
    let path = session_file(root);
    match session {
        None => {
            let _ = std::fs::remove_file(&path);
            Ok(())
        }
        Some(s) => {
            let dir = path.parent().ok_or("bad session path")?;
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let temp = path.with_extension("json.tmp");
            std::fs::write(&temp, serde_json::to_string(s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            std::fs::rename(&temp, &path).map_err(|e| e.to_string())
        }
    }
}

fn trace_flag(root: &Path) -> PathBuf {
    root.join("Library").join("Nodex").join("trace-on")
}

pub fn trace_requested(root: &Path) -> bool {
    trace_flag(root).is_file()
}

/// Asks for scripts with (or without) trace probes; the bridge recompiles.
pub fn set_trace(root: &Path, on: bool) -> Result<(), String> {
    let flag = trace_flag(root);
    if on {
        std::fs::create_dir_all(flag.parent().ok_or("bad path")?).map_err(|e| e.to_string())?;
        std::fs::write(&flag, "Nodex compiles trace probes into this project's scripts while this file exists.\n").map_err(|e| e.to_string())
    } else {
        match std::fs::remove_file(&flag) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The package

/// How the Nodex package is in the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageState {
    Missing,
    /// `"com.nodex.trace": "file:…"` in `Packages/manifest.json`.
    Linked(String),
    /// A copy in `Packages/com.nodex.trace/`, with its version.
    Embedded(String),
}

/// The package folder shipped with Nodex.
pub fn bundled_package() -> Option<PathBuf> {
    let rel = "sidecars/unity/com.nodex.trace/package.json";
    nodex_lang_api::sidecar::locate("NODEX_UNITY_PACKAGE", &[rel], &[rel]).and_then(|p| p.parent().map(Path::to_path_buf))
}

pub fn package_version(dir: &Path) -> Option<String> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("package.json")).ok()?).ok()?;
    v.get("version").and_then(Value::as_str).map(str::to_owned)
}

fn manifest_path(root: &Path) -> PathBuf {
    root.join("Packages").join("manifest.json")
}

pub fn package_state(root: &Path) -> PackageState {
    let embedded = root.join("Packages").join(PACKAGE);
    if embedded.join("package.json").is_file() {
        return PackageState::Embedded(package_version(&embedded).unwrap_or_default());
    }
    let manifest = std::fs::read_to_string(manifest_path(root)).unwrap_or_default();
    let parsed: Value = serde_json::from_str(&manifest).unwrap_or(Value::Null);
    match parsed.pointer(&format!("/dependencies/{PACKAGE}")).and_then(Value::as_str) {
        Some(v) => PackageState::Linked(v.to_owned()),
        None => PackageState::Missing,
    }
}

/// Adds `"<name>": "<value>"` at the end of the manifest's `dependencies`, keeping the rest of the
/// text as it is (Unity projects commit this file). Replaces an existing entry's line.
pub fn manifest_add(text: &str, name: &str, value: &str) -> Result<String, String> {
    let text = manifest_remove(text, name)?;
    let deps = text.find("\"dependencies\"").ok_or("the manifest has no dependencies")?;
    let open = deps + text[deps..].find('{').ok_or("bad manifest")?;
    let close = matching_brace(&text, open).ok_or("bad manifest")?;
    let body = &text[open + 1..close];
    let indent = body
        .lines()
        .find(|l| l.trim_start().starts_with('"'))
        .map(|l| l[..l.len() - l.trim_start().len()].to_owned())
        .unwrap_or_else(|| "    ".into());
    let entry = format!("{indent}{}: {}", json!(name), json!(value));
    let last = body.trim_end();
    let mut out = String::new();
    out.push_str(&text[..open + 1]);
    if last.is_empty() {
        out.push('\n');
        out.push_str(&entry);
    } else {
        out.push_str(last);
        out.push_str(",\n");
        out.push_str(&entry);
    }
    // Keep the whitespace that preceded the closing brace.
    out.push_str(&body[last.len()..]);
    out.push_str(&text[close..]);
    serde_json::from_str::<Value>(&out).map_err(|e| format!("could not edit the manifest: {e}"))?;
    Ok(out)
}

/// Removes the line of `"<name>"` from the manifest's dependencies (and the comma it leaves).
pub fn manifest_remove(text: &str, name: &str) -> Result<String, String> {
    let key = json!(name).to_string();
    let Some(at) = text.find(&format!("{key}:")).or_else(|| text.find(&format!("{key} :"))) else {
        return Ok(text.to_owned());
    };
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[at..].find('\n').map_or(text.len(), |i| at + i + 1);
    let line = &text[line_start..line_end];
    let had_comma = line.trim_end().ends_with(',');
    let mut out = format!("{}{}", &text[..line_start], &text[line_end..]);
    if !had_comma {
        // It was the last entry: the previous one loses its comma.
        if let Some(comma) = out[..line_start].rfind(',')
            && out[comma + 1..line_start].trim().is_empty()
        {
            out.remove(comma);
        }
    }
    serde_json::from_str::<Value>(&out).map_err(|e| format!("could not edit the manifest: {e}"))?;
    Ok(out)
}

fn matching_brace(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in text[open..].char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + i);
                }
            }
            _ => {}
        }
    }
    None
}

/// `file:` reference to Nodex's own copy of the package.
pub fn link_package(root: &Path) -> Result<(), String> {
    let source = bundled_package().ok_or("the Nodex Unity package was not found next to Nodex")?;
    let embedded = root.join("Packages").join(PACKAGE);
    if embedded.exists() {
        return Err("the package is embedded in this project; remove it first".into());
    }
    let source = plain(&source);
    let value = format!("file:{}", source.to_string_lossy().replace('\\', "/"));
    let path = manifest_path(root);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    std::fs::write(&path, manifest_add(&text, PACKAGE, &value)?).map_err(|e| e.to_string())
}

/// A copy of the package in `Packages/com.nodex.trace/` (replaces an older copy).
pub fn embed_package(root: &Path) -> Result<(), String> {
    let source = bundled_package().ok_or("the Nodex Unity package was not found next to Nodex")?;
    let target = root.join("Packages").join(PACKAGE);
    // A linked package would be found twice.
    let path = manifest_path(root);
    if let Ok(text) = std::fs::read_to_string(&path) {
        let cleaned = manifest_remove(&text, PACKAGE)?;
        if cleaned != text {
            std::fs::write(&path, cleaned).map_err(|e| e.to_string())?;
        }
    }
    if target.exists() {
        std::fs::remove_dir_all(&target).map_err(|e| format!("cannot replace {}: {e}", target.display()))?;
    }
    copy_dir(&source, &target)
}

pub fn remove_package(root: &Path) -> Result<(), String> {
    let path = manifest_path(root);
    if let Ok(text) = std::fs::read_to_string(&path) {
        let cleaned = manifest_remove(&text, PACKAGE)?;
        if cleaned != text {
            std::fs::write(&path, cleaned).map_err(|e| e.to_string())?;
        }
    }
    let target = root.join("Packages").join(PACKAGE);
    if target.exists() {
        std::fs::remove_dir_all(&target).map_err(|e| format!("cannot remove {}: {e}", target.display()))?;
    }
    let _ = set_trace(root, false);
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let p = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        if p.is_dir() {
            copy_dir(&p, &to.join(&name))?;
        } else {
            std::fs::copy(&p, to.join(&name)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Without the Windows verbatim prefix (Unity and adapters compare paths as text).
fn plain(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => p.to_path_buf(),
    }
}

fn require_package(root: &Path) -> Result<(), String> {
    if package_state(root) == PackageState::Missing {
        return Err("the Nodex package is not in this Unity project yet: add it from the Unity card (Link or Embed)".into());
    }
    Ok(())
}

fn require_editor(root: &Path) -> Result<EditorInstance, String> {
    editor(root).ok_or_else(|| "open this project in the Unity Editor first: Nodex runs it there".to_owned())
}

// ---------------------------------------------------------------------------------------------
// Live traces: an attached run

/// Every method of the trace maps the Editor's last compilation wrote.
pub fn merged_methods(root: &Path) -> Vec<Value> {
    let mut methods = Vec::new();
    let Ok(entries) = std::fs::read_dir(root.join("Library").join("Nodex").join("maps")) else { return methods };
    let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
    files.sort();
    for f in files {
        if let Some(list) = std::fs::read_to_string(&f).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|v| v.get("methods").cloned())
            && let Value::Array(list) = list
        {
            methods.extend(list);
        }
    }
    methods
}

pub struct UnityInstrumenter;

impl TraceInstrumenter for UnityInstrumenter {
    fn prepare(&self, root: &Path, out_dir: &Path, env: &RunEnv, log: &mut dyn FnMut(&str)) -> Result<RunCommand, String> {
        require_package(root)?;
        let instance = require_editor(root)?;
        let fresh = !trace_requested(root);
        set_trace(root, true)?;
        let session = Session {
            id: env.token.chars().take(12).collect(),
            endpoint: env.tcp.clone(),
            token: env.token.clone(),
            play: true,
            debug: read_session(root).is_some_and(|s| s.debug),
        };
        write_session(root, Some(&session))?;
        if fresh {
            log("Unity recompiles the project's scripts with trace probes (first time) …");
        }
        log(&format!("Asked Unity {} (pid {}) to enter Play Mode", instance.version, instance.process_id));
        let dir = out_dir.join("unity");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let map = dir.join("nodex-trace-map.json");
        std::fs::write(&map, json!({ "version": 1, "methods": merged_methods(root) }).to_string()).map_err(|e| e.to_string())?;
        Ok(RunCommand {
            program: String::new(),
            args: Vec::new(),
            cwd: root.to_path_buf(),
            trace_map: map,
            description: format!("Unity {} Play Mode (instrumented)", instance.version),
            kind: RunKind::Attached,
            env: Vec::new(),
        })
    }

    fn refresh(&self, root: &Path, _out_dir: &Path) -> Option<Result<String, String>> {
        Some(Ok(Value::Array(merged_methods(root)).to_string()))
    }

    fn finish(&self, root: &Path) {
        // Leave Play Mode; keep a debug request if a debugger is attached.
        let keep_debug = read_session(root).is_some_and(|s| s.debug);
        let next = keep_debug.then(|| Session { id: "debug".into(), debug: true, ..Default::default() });
        let _ = write_session(root, next.as_ref());
    }
}

// ---------------------------------------------------------------------------------------------
// Debugging: unity-dap attached to the Editor

pub struct UnityDebugger;

const DOWNLOADS: &[AdapterDownload] = &[
    AdapterDownload {
        os: "windows",
        arch: "x86_64",
        url: "https://github.com/walcht/unity-dap/releases/download/v0.1.0/unity-debug-adapter-v0.1.0-win-x64.zip",
        sha256: "ca59dd3d1a079add7b8beab86596b5c79c1ea567698a2075a5ad74fa10b10978",
        size: 6_418_246,
        format: "zip",
        program: "unity-debug-adapter.exe",
    },
    AdapterDownload {
        os: "linux",
        arch: "x86_64",
        url: "https://github.com/walcht/unity-dap/releases/download/v0.1.0/unity-debug-adapter-v0.1.0-linux-x64.zip",
        sha256: "013600b650b260918d1f8b7835d167583f84b213d96087b42870ba6292447270",
        size: 6_458_608,
        format: "zip",
        program: "unity-debug-adapter.exe",
    },
    AdapterDownload {
        os: "macos",
        arch: "x86_64",
        url: "https://github.com/walcht/unity-dap/releases/download/v0.1.0/unity-debug-adapter-v0.1.0-osx-x64.zip",
        sha256: "1f8ed54ef39b6a9e38df67e8b02dedf0e04d3e357273c5ae688e5085fa8891e7",
        size: 6_455_118,
        format: "zip",
        program: "unity-debug-adapter.exe",
    },
];

impl DebugLauncher for UnityDebugger {
    fn adapter(&self) -> AdapterSpec {
        AdapterSpec {
            name: "unity-dap",
            version: "0.1.0",
            adapter_id: "unity",
            homepage: "https://github.com/walcht/unity-dap",
            downloads: DOWNLOADS,
            env_override: "NODEX_UNITY_DAP",
            // A .NET Framework 4.7.2 program: Windows runs it; elsewhere it needs Mono.
            interpreter: if cfg!(windows) { None } else { Some("mono") },
            args: &[],
            transport: AdapterTransport::Stdio,
        }
    }

    fn prepare(&self, root: &Path, _out_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<DebugLaunch, String> {
        let instance = require_editor(root)?;
        if package_state(root) != PackageState::Missing {
            // The bridge switches the Editor to Debug code optimization (needed to attach).
            let mut session = read_session(root).unwrap_or_else(|| Session { id: "debug".into(), ..Default::default() });
            session.debug = true;
            write_session(root, Some(&session))?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
            let mut said = false;
            loop {
                match bridge(root) {
                    Some(b) if b.code_optimization == "Debug" && !b.compiling => break,
                    Some(_) if !said => {
                        log("Switching the Unity Editor to Debug mode (it recompiles) …");
                        said = true;
                    }
                    None if std::time::Instant::now() > deadline - std::time::Duration::from_secs(50) => {
                        // No bridge report: the package is not loaded yet; try to attach anyway.
                        break;
                    }
                    _ => {}
                }
                if std::time::Instant::now() > deadline {
                    return Err("the Unity Editor did not switch to Debug mode in time".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
        } else {
            log("Without the Nodex package, switch the Editor to Debug mode yourself (the bug icon in its status bar)");
        }
        let port = debugger_port(instance.process_id);
        Ok(DebugLaunch {
            launch: json!({
                "name": "Nodex",
                "type": "unity",
                "request": "attach",
                "address": "127.0.0.1",
                "port": port,
            }),
            description: format!("Unity {} Editor (pid {}, port {port})", instance.version, instance.process_id),
            attach: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = "{\n  \"dependencies\": {\n    \"com.unity.ugui\": \"2.0.0\",\n    \"com.unity.modules.ai\": \"1.0.0\"\n  }\n}\n";

    #[test]
    fn manifest_entries_are_added_and_removed_in_place() {
        let added = manifest_add(MANIFEST, PACKAGE, "file:D:/nodex/com.nodex.trace").unwrap();
        assert_eq!(
            added,
            "{\n  \"dependencies\": {\n    \"com.unity.ugui\": \"2.0.0\",\n    \"com.unity.modules.ai\": \"1.0.0\",\n    \"com.nodex.trace\": \"file:D:/nodex/com.nodex.trace\"\n  }\n}\n"
        );
        // Adding again replaces the line.
        let again = manifest_add(&added, PACKAGE, "file:E:/other").unwrap();
        assert_eq!(again.matches(PACKAGE).count(), 1);
        assert!(again.contains("file:E:/other"));
        assert_eq!(manifest_remove(&added, PACKAGE).unwrap(), MANIFEST);
        // Removing an entry in the middle keeps the others valid.
        let middle = manifest_remove(MANIFEST, "com.unity.ugui").unwrap();
        assert_eq!(middle, "{\n  \"dependencies\": {\n    \"com.unity.modules.ai\": \"1.0.0\"\n  }\n}\n");
        assert_eq!(manifest_remove(MANIFEST, PACKAGE).unwrap(), MANIFEST);
    }

    #[test]
    fn unity_projects_are_named_after_their_product() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/unity-sample");
        assert!(is_unity(&sample));
        assert_eq!(crate::project::detect(&sample).unwrap().name, "Sample");
    }

    #[test]
    fn editor_ports_follow_the_process_id() {
        assert_eq!(debugger_port(12345), 56345);
        assert_eq!(debugger_port(1000), 56000);
    }

    #[test]
    fn sessions_round_trip() {
        let dir = std::env::temp_dir().join(format!("nodex-unity-session-{}", std::process::id()));
        let s = Session { id: "abc".into(), endpoint: "127.0.0.1:1".into(), token: "t".into(), play: true, debug: false };
        write_session(&dir, Some(&s)).unwrap();
        assert_eq!(read_session(&dir), Some(s));
        write_session(&dir, None).unwrap();
        assert_eq!(read_session(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
