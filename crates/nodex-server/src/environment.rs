//! What this machine has for each language: the toolchains Nodex calls (the .NET SDK, Node.js,
//! Cargo, rust-analyzer, the Unity Editor) and the helpers shipped with Nodex itself. The UI
//! shows it so a missing tool is named, with what it enables and where to get it.

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

/// One tool or bundled component.
#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct ToolStatus {
    /// Stable id: `dotnet`, `node`, `cargo`, `rust-analyzer`, `unity`, `roslyn`, `weaver`,
    /// `nodex-ts`, `rust-trace-runtime`, `unity-package`.
    pub id: String,
    pub name: String,
    /// `toolchain` (installed by the user) or `bundled` (shipped with Nodex).
    pub kind: String,
    pub found: bool,
    /// Version text reported by the tool.
    pub version: Option<String>,
    pub path: Option<String>,
    /// What it enables in Nodex.
    pub enables: String,
    /// Where to get it (toolchains only).
    pub install_url: Option<String>,
    /// Why it is missing, or how to install it.
    pub hint: Option<String>,
}

#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct EnvironmentReport {
    pub tools: Vec<ToolStatus>,
    /// Nodex's version.
    pub version: String,
    /// Folder of the log files.
    pub logs: String,
}

fn tool(id: &str, name: &str, kind: &str, enables: &str, install_url: Option<&str>) -> ToolStatus {
    ToolStatus {
        id: id.into(),
        name: name.into(),
        kind: kind.into(),
        found: false,
        version: None,
        path: None,
        enables: enables.into(),
        install_url: install_url.map(str::to_owned),
        hint: None,
    }
}

/// First line of `program args…` when it succeeds.
fn first_line(program: impl AsRef<std::ffi::OsStr>, args: &[&str]) -> Option<String> {
    let out = nodex_lang_api::process::command(program).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or("").trim().to_owned())
}

fn dotnet() -> ToolStatus {
    let mut t = tool(
        "dotnet",
        ".NET SDK",
        "toolchain",
        "Exact C# calls, events and types (Roslyn), ▶ Run with live traces and debugging of .NET programs",
        Some("https://dotnet.microsoft.com/download"),
    );
    let sdks = nodex_lang_api::process::command("dotnet").arg("--list-sdks").output();
    match sdks {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            // `10.0.100 [C:\Program Files\dotnet\sdk]`, oldest first.
            let versions: Vec<&str> = text.lines().filter_map(|l| l.split_whitespace().next()).collect();
            if let Some(newest) = versions.last() {
                t.found = true;
                t.version = Some(versions.join(", "));
                let major = newest.split('.').next().and_then(|m| m.parse::<u32>().ok()).unwrap_or(0);
                if major < 10 {
                    t.hint = Some(format!("Nodex's C# helpers need .NET 10 or newer; the newest SDK here is {newest}"));
                }
            } else {
                t.hint = Some("dotnet is installed but has no SDK (only a runtime)".into());
            }
        }
        _ => t.hint = Some("dotnet was not found on the PATH".into()),
    }
    t
}

fn node() -> ToolStatus {
    let mut t = tool(
        "node",
        "Node.js",
        "toolchain",
        "Exact TypeScript/JavaScript calls and types, ▶ Run and debugging of Node and browser apps",
        Some("https://nodejs.org/en/download"),
    );
    let program = nodex_lang_typescript::semantic::node();
    match first_line(&program, &["--version"]) {
        Some(v) => {
            t.found = true;
            t.version = Some(v);
            t.path = Some(program.display().to_string());
        }
        None => t.hint = Some(format!("{} was not found (set NODEX_NODE to use another one)", program.display())),
    }
    t
}

fn cargo() -> ToolStatus {
    let mut t = tool("cargo", "Rust (Cargo)", "toolchain", "▶ Run and debugging of Rust programs", Some("https://rustup.rs"));
    match first_line("cargo", &["--version"]) {
        Some(v) => {
            t.found = true;
            t.version = Some(v);
        }
        None => t.hint = Some("cargo was not found on the PATH; install Rust with rustup".into()),
    }
    t
}

fn rust_analyzer() -> ToolStatus {
    let mut t = tool(
        "rust-analyzer",
        "rust-analyzer",
        "toolchain",
        "Exact Rust calls and types",
        Some("https://rust-analyzer.github.io/book/installation.html"),
    );
    match nodex_lang_rust::semantic::find_binary() {
        Some((path, version)) => {
            t.found = true;
            t.version = Some(version);
            t.path = Some(path.display().to_string());
        }
        None => t.hint = Some("run `rustup component add rust-analyzer` (or set NODEX_RUST_ANALYZER)".into()),
    }
    t
}

/// Unity Hub's editor folders: the one chosen in its settings (`secondaryInstallPath.json`, a
/// JSON string) and the default ones.
fn unity_editor_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let hub_config = if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("UnityHub"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support/UnityHub"))
    } else {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/UnityHub"))
    };
    if let Some(chosen) = hub_config
        .and_then(|c| std::fs::read_to_string(c.join("secondaryInstallPath.json")).ok())
        .and_then(|t| serde_json::from_str::<String>(&t).ok())
        .filter(|p| !p.trim().is_empty())
    {
        dirs.push(PathBuf::from(chosen));
    }
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramW6432"] {
            if let Some(p) = std::env::var_os(var) {
                dirs.push(PathBuf::from(p).join("Unity").join("Hub").join("Editor"));
            }
        }
    } else if cfg!(target_os = "macos") {
        dirs.push(PathBuf::from("/Applications/Unity/Hub/Editor"));
    } else if let Some(h) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(h).join("Unity").join("Hub").join("Editor"));
    }
    dirs.dedup();
    dirs
}

fn unity() -> ToolStatus {
    let mut t = tool(
        "unity",
        "Unity Editor",
        "toolchain",
        "Play Mode live traces and debugging of Unity projects",
        Some("https://unity.com/download"),
    );
    let mut versions: Vec<String> = unity_editor_dirs()
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flatten()
        .filter_map(Result::ok)
        // An editor folder holds the Editor (Windows/Linux) or Unity.app (macOS).
        .filter(|e| e.path().join("Editor").is_dir() || e.path().join("Unity.app").is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    versions.sort();
    versions.dedup();
    if versions.is_empty() {
        t.hint = Some("no editor found in Unity Hub's install folders (one installed elsewhere still works)".into());
    } else {
        t.found = true;
        t.version = Some(versions.join(", "));
    }
    t
}

fn bundled(id: &str, name: &str, enables: &str, found: Option<PathBuf>) -> ToolStatus {
    let mut t = tool(id, name, "bundled", enables, None);
    match found {
        Some(p) => {
            t.found = true;
            t.path = Some(p.display().to_string());
        }
        None => t.hint = Some("missing from this installation; reinstall Nodex".into()),
    }
    t
}

fn components() -> Vec<ToolStatus> {
    let parent = |p: PathBuf| p.parent().map(Path::to_path_buf);
    vec![
        bundled("roslyn", "Roslyn helper", "Exact C# semantics", nodex_lang_csharp::roslyn::locate()),
        bundled(
            "weaver",
            "IL weaver",
            "Live traces of .NET programs",
            nodex_lang_csharp::sidecar::locate("Nodex.Weaver", "NODEX_WEAVER"),
        ),
        bundled(
            "nodex-ts",
            "TypeScript helper",
            "Exact TypeScript semantics and traces",
            nodex_lang_typescript::semantic::locate().and_then(parent).and_then(parent),
        ),
        bundled("rust-trace-runtime", "Rust trace runtime", "Live traces of Rust programs", nodex_lang_rust::instrument::runtime_dir()),
        bundled("unity-package", "Unity package", "Unity Play Mode traces", nodex_lang_csharp::unity_editor::bundled_package()),
    ]
}

/// Checks every tool (in parallel: each one starts a process).
pub fn report() -> EnvironmentReport {
    let checks: [fn() -> ToolStatus; 5] = [dotnet, node, cargo, rust_analyzer, unity];
    let mut tools: Vec<ToolStatus> = std::thread::scope(|s| {
        let handles: Vec<_> = checks.iter().map(|f| s.spawn(f)).collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    tools.extend(components());
    EnvironmentReport {
        tools,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        logs: crate::logging::logs_dir().display().to_string(),
    }
}
