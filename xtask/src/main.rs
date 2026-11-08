//! Nodex build tasks.
//!
//! `cargo xtask dist` builds everything the desktop app ships and the Windows installer:
//!
//! 1. the web UI (`web/dist`, embedded in the binary);
//! 2. the sidecars, staged in `target/nodex-dist/resources/` with the layout the locators expect
//!    under the app's resource folder (`sidecars/Nodex.Roslyn/Nodex.Roslyn.dll`, …);
//! 3. the sample projects (`samples/`);
//! 4. the installer, with `cargo tauri build` and `crates/nodex-desktop/tauri.bundle.json`, which
//!    adds the staged resources (kept out of `tauri.conf.json` so plain `cargo build` works
//!    without them).
//!
//! Options: `--no-bundle` stops after staging; `--skip-web` reuses `web/dist`.
//! Needs pnpm, npm, the .NET SDK and the Tauri CLI (`cargo install tauri-cli --version "^2" --locked`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Folders never copied from the source tree (build output, local state).
const SKIP: &[&str] = &["bin", "obj", ".nodex", "target", "node_modules", "Library", "Temp", "Logs"];

/// Sample projects: (folder in the source tree, name under `samples/`).
const SAMPLES: &[(&str, &str)] = &[("fixtures/csharp/Sample", "orders"), ("fixtures/rosetta/filter-map-sum", "rosetta")];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("dist") => dist(&args[1..]),
        _ => {
            eprintln!("usage: cargo xtask dist [--no-bundle] [--skip-web]");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("\nerror: {e}");
            ExitCode::FAILURE
        }
    }
}

type Result<T = ()> = std::result::Result<T, String>;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask lives in the repository").to_path_buf()
}

fn dist(args: &[String]) -> Result {
    let bundle = !args.iter().any(|a| a == "--no-bundle");
    let web = !args.iter().any(|a| a == "--skip-web");
    let root = root();
    let stage = root.join("target").join("nodex-dist").join("resources");
    if stage.exists() {
        fs::remove_dir_all(&stage).map_err(|e| format!("cannot clear {}: {e}", stage.display()))?;
    }
    let sidecars = stage.join("sidecars");

    if web {
        step("Web UI");
        run(tool("pnpm").args(["--dir", "web", "install", "--frozen-lockfile"]).current_dir(&root))?;
        run(tool("pnpm").args(["--dir", "web", "build"]).current_dir(&root))?;
    }

    step(".NET sidecars (Roslyn, weaver)");
    for name in ["Nodex.Roslyn", "Nodex.Weaver"] {
        let out = sidecars.join(name);
        run(Command::new("dotnet")
            .arg("publish")
            .arg(root.join("sidecars/dotnet").join(name))
            .args(["-c", "Release", "--nologo", "-p:SatelliteResourceLanguages=en", "-p:DebugType=none", "-o"])
            .arg(&out))?;
        require(&out.join(format!("{name}.dll")))?;
    }

    step("TypeScript sidecar");
    let ts_src = root.join("sidecars/node/nodex-ts");
    run(tool("pnpm").args(["install", "--frozen-lockfile"]).current_dir(&ts_src))?;
    run(tool("pnpm").args(["run", "build"]).current_dir(&ts_src))?;
    let ts_out = sidecars.join("nodex-ts");
    copy_dir(&ts_src.join("dist"), &ts_out.join("dist"))?;
    fs::copy(ts_src.join("package.json"), ts_out.join("package.json")).map_err(|e| e.to_string())?;
    // A flat node_modules (pnpm's links do not survive copying): the compiler and its lib .d.ts.
    run(tool("npm").args(["install", "--omit=dev", "--no-package-lock", "--no-audit", "--no-fund", "--ignore-scripts"]).current_dir(&ts_out))?;
    require(&ts_out.join("dist/main.js"))?;
    require(&ts_out.join("node_modules/typescript/lib/lib.d.ts"))?;
    // nodex-ts finds the trace runtimes next to its own folder.
    copy_dir(&root.join("sidecars/node/nodex-trace"), &sidecars.join("nodex-trace"))?;

    step("Trace runtimes (Rust crate, Unity package)");
    let rt = sidecars.join("rust/nodex-trace-rt");
    fs::create_dir_all(&rt).map_err(|e| e.to_string())?;
    fs::copy(root.join("sidecars/rust/nodex-trace-rt/Cargo.toml"), rt.join("Cargo.toml")).map_err(|e| e.to_string())?;
    copy_dir(&root.join("sidecars/rust/nodex-trace-rt/src"), &rt.join("src"))?;
    copy_dir(&root.join("sidecars/unity/com.nodex.trace"), &sidecars.join("unity/com.nodex.trace"))?;

    step("Samples");
    for (src, name) in SAMPLES {
        let out = stage.join("samples").join(name);
        copy_dir(&root.join(src), &out)?;
        // Test expectations of the Rosetta fixtures are not part of the sample.
        for entry in fs::read_dir(&out).map_err(|e| e.to_string())?.flatten() {
            if entry.file_name().to_string_lossy().starts_with("expected.") {
                fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
            }
        }
    }

    println!("\nStaged resources in {}", stage.display());
    if !bundle {
        return Ok(());
    }

    step("Installer (cargo tauri build)");
    let desktop = root.join("crates/nodex-desktop");
    let has_cli = Command::new("cargo").args(["tauri", "--version"]).output().is_ok_and(|o| o.status.success());
    if !has_cli {
        return Err("the Tauri CLI is missing: cargo install tauri-cli --version \"^2\" --locked".into());
    }
    run(Command::new("cargo")
        .args(["tauri", "build", "--config", "tauri.bundle.json"])
        .env("CARGO_ENCODED_RUSTFLAGS", remap_flags(&root))
        .current_dir(&desktop))?;
    let nsis = root.join("target/release/bundle/nsis");
    for entry in fs::read_dir(&nsis).map_err(|e| format!("no installer in {}: {e}", nsis.display()))?.flatten() {
        println!("Installer: {}", entry.path().display());
    }
    Ok(())
}

/// `--remap-path-prefix` flags so the shipped binary's panic locations name `/cargo`, `/rustup`
/// and `/nodex` instead of this machine's folders (and its user name). Separated by 0x1f, as
/// `CARGO_ENCODED_RUSTFLAGS` expects (paths may contain spaces).
fn remap_flags(root: &Path) -> String {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    let cargo_home = std::env::var_os("CARGO_HOME").map(PathBuf::from).or_else(|| home.as_ref().map(|h| h.join(".cargo")));
    let rustup_home = std::env::var_os("RUSTUP_HOME").map(PathBuf::from).or_else(|| home.as_ref().map(|h| h.join(".rustup")));
    let mut flags = Vec::new();
    for (from, to) in [(cargo_home, "/cargo"), (rustup_home, "/rustup"), (Some(root.to_path_buf()), "/nodex")] {
        if let Some(from) = from {
            flags.push(format!("--remap-path-prefix={}={to}", from.display()));
        }
    }
    flags.join("\x1f")
}

fn step(name: &str) {
    println!("\n==> {name}");
}

/// A command for a Node tool: `pnpm` and `npm` are `.cmd` scripts on Windows.
fn tool(name: &str) -> Command {
    if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", name]);
        c
    } else {
        Command::new(name)
    }
}

fn run(cmd: &mut Command) -> Result {
    let shown = format!("{cmd:?}");
    let status = cmd.status().map_err(|e| format!("cannot run {shown}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{shown} failed ({status})")) }
}

fn require(path: &Path) -> Result {
    if path.exists() { Ok(()) } else { Err(format!("expected {} after the build", path.display())) }
}

/// Copies a folder recursively, skipping build output and local state.
fn copy_dir(from: &Path, to: &Path) -> Result {
    fs::create_dir_all(to).map_err(|e| format!("cannot create {}: {e}", to.display()))?;
    for entry in fs::read_dir(from).map_err(|e| format!("cannot read {}: {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        let target = to.join(&name);
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            if SKIP.iter().any(|s| name == *s) {
                continue;
            }
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target).map_err(|e| format!("cannot copy {}: {e}", entry.path().display()))?;
        }
    }
    Ok(())
}
