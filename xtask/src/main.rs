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
//!
//! With `TAURI_SIGNING_PRIVATE_KEY` set (the release workflow), it also signs the installer for
//! the updater and writes `target/release/bundle/latest.json`, the update manifest.
//!
//! `cargo xtask version <x.y.z>` sets the version everywhere it is written; `cargo xtask notes
//! <x.y.z>` prints that version's CHANGELOG section (the release notes).

mod licenses;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Folders never copied from the source tree (build output, local state).
const SKIP: &[&str] = &["bin", "obj", ".nodex", "target", "node_modules", "Library", "Temp", "Logs"];

/// Where releases are published (the updater reads `latest.json` from the latest one).
const RELEASES: &str = "https://github.com/LeGustaVinho/nodex/releases";

/// Sample projects: (folder in the source tree, name under `samples/`).
const SAMPLES: &[(&str, &str)] = &[("fixtures/csharp/Sample", "orders"), ("fixtures/rosetta/filter-map-sum", "rosetta")];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("dist") => dist(&args[1..]),
        Some("version") if args.len() == 2 => set_version(&args[1]),
        Some("notes") if args.len() == 2 => release_notes(&args[1]).map(|n| println!("{n}")),
        Some("licenses") => {
            let stage = stage_dir(&root());
            licenses::write(&root(), &stage, &stage.join("THIRD-PARTY-NOTICES.txt"))
        }
        _ => {
            eprintln!("usage: cargo xtask dist [--no-bundle] [--skip-web]");
            eprintln!("       cargo xtask version <x.y.z>");
            eprintln!("       cargo xtask notes <x.y.z>");
            eprintln!("       cargo xtask licenses   (after dist --no-bundle)");
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

/// Where `dist` stages what the installer adds to the app's resource folder.
fn stage_dir(root: &Path) -> PathBuf {
    root.join("target").join("nodex-dist").join("resources")
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask lives in the repository").to_path_buf()
}

fn dist(args: &[String]) -> Result {
    let bundle = !args.iter().any(|a| a == "--no-bundle");
    let web = !args.iter().any(|a| a == "--skip-web");
    let root = root();
    let stage = stage_dir(&root);
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

    step("Licenses");
    fs::copy(root.join("LICENSE"), stage.join("LICENSE.txt")).map_err(|e| format!("cannot copy LICENSE: {e}"))?;
    licenses::write(&root, &stage, &stage.join("THIRD-PARTY-NOTICES.txt"))?;

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
    let nsis = root.join("target/release/bundle/nsis");
    // Installers of other versions would be published too.
    let _ = fs::remove_dir_all(&nsis);
    let signed = std::env::var_os("TAURI_SIGNING_PRIVATE_KEY").is_some_and(|k| !k.is_empty());
    let mut tauri = Command::new("cargo");
    tauri.args(["tauri", "build", "--config", "tauri.bundle.json"]);
    if signed {
        tauri.args(["--config", r#"{"bundle":{"createUpdaterArtifacts":true}}"#]);
    }
    // Authenticode (for example Azure Trusted Signing): a command Tauri runs on each binary, with
    // `%1` for the file. Unset, the installer is not code-signed (SmartScreen warns).
    if let Some(sign) = std::env::var("WINDOWS_SIGN_COMMAND").ok().filter(|c| !c.trim().is_empty()) {
        tauri.args(["--config", &format!(r#"{{"bundle":{{"windows":{{"signCommand":{}}}}}}}"#, json_string(&sign))]);
    }
    run(tauri.env("CARGO_ENCODED_RUSTFLAGS", remap_flags(&root)).current_dir(&desktop))?;

    let version = workspace_version(&root)?;
    let installer = nsis.join(format!("Nodex_{version}_x64-setup.exe"));
    require(&installer)?;
    println!("Installer: {}", installer.display());
    if signed {
        let sig_path = nsis.join(format!("Nodex_{version}_x64-setup.exe.sig"));
        let signature = fs::read_to_string(&sig_path).map_err(|e| format!("no updater signature at {}: {e}", sig_path.display()))?;
        let manifest = update_manifest(&version, &release_notes(&version).unwrap_or_default(), signature.trim());
        let path = root.join("target/release/bundle/latest.json");
        fs::write(&path, manifest).map_err(|e| e.to_string())?;
        println!("Update manifest: {}", path.display());
    }
    Ok(())
}

/// `latest.json` for tauri-plugin-updater: this version, its notes, and the signed installer
/// in the GitHub release `v<version>`.
fn update_manifest(version: &str, notes: &str, signature: &str) -> String {
    let url = format!("{RELEASES}/download/v{version}/Nodex_{version}_x64-setup.exe");
    let mut out = String::from("{\n");
    out.push_str(&format!("  \"version\": {},\n", json_string(version)));
    out.push_str(&format!("  \"notes\": {},\n", json_string(notes)));
    out.push_str(&format!("  \"pub_date\": \"{}\",\n", utc_now_rfc3339()));
    out.push_str("  \"platforms\": {\n");
    out.push_str(&format!(
        "    \"windows-x86_64\": {{ \"signature\": {}, \"url\": {} }}\n",
        json_string(signature),
        json_string(&url)
    ));
    out.push_str("  }\n}\n");
    out
}

fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The current time as `YYYY-MM-DDTHH:MM:SSZ`.
fn utc_now_rfc3339() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// The version in the workspace `Cargo.toml` (`[workspace.package] version`).
fn workspace_version(root: &Path) -> Result<String> {
    let text = fs::read_to_string(root.join("Cargo.toml")).map_err(|e| e.to_string())?;
    let section = text.split("[workspace.package]").nth(1).ok_or("no [workspace.package] in Cargo.toml")?;
    section
        .lines()
        .find_map(|l| l.trim().strip_prefix("version = \"").and_then(|v| v.strip_suffix('"')))
        .map(str::to_owned)
        .ok_or_else(|| "no version in [workspace.package]".into())
}

/// The CHANGELOG section of `version`, without its heading.
fn release_notes(version: &str) -> Result<String> {
    let text = fs::read_to_string(root().join("CHANGELOG.md")).map_err(|e| e.to_string())?;
    let text = text.replace("\r\n", "\n");
    let heading = format!("## [{version}]");
    let start = text.find(&heading).ok_or_else(|| format!("CHANGELOG.md has no section {heading}"))?;
    let body = text[start..].split_once('\n').map(|(_, rest)| rest).unwrap_or("");
    let end = body.find("\n## ").or_else(|| body.find("\n[")).unwrap_or(body.len());
    Ok(body[..end].trim().to_owned())
}

/// Writes `version` into every file that carries Nodex's version.
fn set_version(version: &str) -> Result {
    let valid = version.split('-').next().is_some_and(|core| {
        let parts: Vec<&str> = core.split('.').collect();
        parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    });
    if !valid {
        return Err(format!("{version} is not a version like 1.2.3"));
    }
    let root = root();
    let old = workspace_version(&root)?;
    let json = |v: &str| format!("\"version\": \"{v}\"");
    let files = [
        ("Cargo.toml", format!("version = \"{old}\""), format!("version = \"{version}\"")),
        ("web/package.json", json(&old), json(version)),
        ("sidecars/node/nodex-ts/package.json", json(&old), json(version)),
        ("sidecars/unity/com.nodex.trace/package.json", json(&old), json(version)),
        ("sidecars/unity/com.nodex.trace/Editor/NodexBridge.cs", format!("Version = \"{old}\""), format!("Version = \"{version}\"")),
    ];
    // Check every file before changing any.
    let mut texts = Vec::new();
    for (file, from, _) in &files {
        let text = fs::read_to_string(root.join(file)).map_err(|e| format!("cannot read {file}: {e}"))?;
        if !text.contains(from.as_str()) {
            return Err(format!("{file} does not contain {from}; is every version {old}?"));
        }
        texts.push(text);
    }
    for ((file, from, to), text) in files.iter().zip(texts) {
        fs::write(root.join(file), text.replacen(from.as_str(), to, 1)).map_err(|e| format!("cannot write {file}: {e}"))?;
        println!("{file}: {old} -> {version}");
    }
    println!("Now refresh Cargo.lock (cargo check) and move the CHANGELOG notes under ## [{version}].");
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
