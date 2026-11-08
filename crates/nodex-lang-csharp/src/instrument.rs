//! Live-trace preparation for .NET: `dotnet build`, then `Nodex.Weaver` on the output.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use nodex_lang_api::{RunCommand, RunEnv, RunKind, TraceInstrumenter};

use crate::sidecar;

const SKIP_DIRS: &[&str] = &["bin", "obj", ".git", "node_modules", "Library", "Temp", "Logs", ".nodex", "target"];

pub struct DotnetInstrumenter;

impl TraceInstrumenter for DotnetInstrumenter {
    fn prepare(&self, root: &Path, out_dir: &Path, _env: &RunEnv, log: &mut dyn FnMut(&str)) -> Result<RunCommand, String> {
        let project = find_executable_project(root)?;
        let weaver = sidecar::locate("Nodex.Weaver", "NODEX_WEAVER")
            .ok_or("Nodex.Weaver not found; build it with `dotnet build sidecars/dotnet/Nodex.Weaver -c Release` or set NODEX_WEAVER")?;
        let build = out_dir.join("build");
        let traced = out_dir.join("traced");
        for dir in [&build, &traced] {
            if dir.exists() {
                std::fs::remove_dir_all(dir).map_err(|e| format!("cannot clean {}: {e}", dir.display()))?;
            }
        }

        log(&format!("Building {} …", project.file_name().unwrap_or_default().to_string_lossy()));
        let mut cmd = nodex_lang_api::process::command("dotnet");
        cmd.arg("build").arg(&project).args(["-c", "Debug", "-nologo", "-v", "minimal", "-o"]).arg(&build);
        run_logged(cmd, log).map_err(|e| format!("build failed: {e}"))?;

        log("Instrumenting (Nodex.Weaver) …");
        let mut cmd = nodex_lang_api::process::command("dotnet");
        cmd.arg(&weaver).arg("--root").arg(root).arg("--input").arg(&build).arg("--output").arg(&traced);
        run_logged(cmd, log).map_err(|e| format!("instrumentation failed: {e}"))?;

        let app = entry_assembly(&build, &project)?;
        Ok(RunCommand {
            program: "dotnet".into(),
            args: vec![traced.join(format!("{app}.dll")).to_string_lossy().into_owned()],
            cwd: project.parent().unwrap_or(root).to_path_buf(),
            trace_map: traced.join("nodex-trace-map.json"),
            description: format!("{app} (.NET, instrumented)"),
            kind: RunKind::Program,
            env: Vec::new(),
        })
    }
}

/// Runs a command, forwarding each output line to `log`; returns stdout on success.
pub(crate) fn run_logged(mut cmd: Command, log: &mut dyn FnMut(&str)) -> Result<String, String> {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot start `dotnet` ({e}); is the .NET SDK installed?"))?;
    let stderr = child.stderr.take();
    let err_thread = std::thread::spawn(move || {
        let mut lines = Vec::new();
        if let Some(s) = stderr {
            for line in BufReader::new(s).lines().map_while(Result::ok) {
                lines.push(line);
            }
        }
        lines
    });
    let mut stdout = String::new();
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            log(&line);
            stdout.push_str(&line);
            stdout.push('\n');
        }
    }
    for line in err_thread.join().unwrap_or_default() {
        log(&line);
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() { Ok(stdout) } else { Err(format!("exit code {}", status.code().unwrap_or(-1))) }
}

/// The first `.csproj` (three folders deep) with `<OutputType>Exe</OutputType>`.
pub(crate) fn find_executable_project(root: &Path) -> Result<PathBuf, String> {
    let mut found = Vec::new();
    collect(root, 0, &mut found);
    found.sort();
    found
        .into_iter()
        .find(|p| {
            std::fs::read_to_string(p)
                .map(|t| {
                    let t = t.to_ascii_lowercase().replace(' ', "");
                    t.contains("<outputtype>exe</outputtype>") || t.contains("<outputtype>winexe</outputtype>")
                })
                .unwrap_or(false)
        })
        .ok_or_else(|| "no runnable project found (a .csproj with <OutputType>Exe</OutputType>)".to_owned())
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            let skip = p.file_name().and_then(|n| n.to_str()).is_some_and(|n| SKIP_DIRS.contains(&n));
            if depth < 3 && !skip {
                collect(&p, depth + 1, out);
            }
        } else if p.extension().is_some_and(|x| x == "csproj") {
            out.push(p);
        }
    }
}

/// The app's assembly name, from `<name>.runtimeconfig.json` in the build output.
pub(crate) fn entry_assembly(build: &Path, project: &Path) -> Result<String, String> {
    let stem = project.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let configs: Vec<String> = std::fs::read_dir(build)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".runtimeconfig.json")).map(str::to_owned))
        .collect();
    configs
        .iter()
        .find(|c| **c == stem)
        .or_else(|| configs.first())
        .cloned()
        .ok_or_else(|| "the build produced no runnable app (no .runtimeconfig.json)".to_owned())
}
