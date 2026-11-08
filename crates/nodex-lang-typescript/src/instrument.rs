//! Live-trace preparation through `nodex-ts instrument`:
//! - Node.js programs: an instrumented JavaScript copy of the project, run with the trace
//!   runtime preloaded (`node --require nodex-trace.cjs <entry>`);
//! - browser apps (React, Vue, Angular with a dev server): an instrumented *source* copy that
//!   the project's own dev script serves; the page sends its events over a WebSocket. Files
//!   changed during the run are instrumented again into the copy, so the dev server reloads
//!   them.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use nodex_lang_api::{RunCommand, RunEnv, RunKind, TraceInstrumenter};
use serde_json::Value;

use crate::semantic::{BUILD_HINT, locate, node};

const RESULT_PREFIX: &str = "NODEX-RESULT ";

pub struct NodeInstrumenter;

/// Runs `nodex-ts instrument <args>` and returns its `NODEX-RESULT` JSON.
fn sidecar(args: &[&std::ffi::OsStr], log: &mut dyn FnMut(&str)) -> Result<Value, String> {
    let sidecar = locate().ok_or_else(|| format!("TypeScript sidecar not found; {BUILD_HINT}"))?;
    let mut child = Command::new(node())
        .arg(&sidecar)
        .arg("instrument")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start Node.js ({e}); is it installed?"))?;
    let stderr = child.stderr.take();
    let errors = std::thread::spawn(move || {
        stderr.map(|s| BufReader::new(s).lines().map_while(Result::ok).collect::<Vec<_>>()).unwrap_or_default()
    });
    let mut result = None;
    if let Some(stdout) = child.stdout.take() {
        for l in BufReader::new(stdout).lines().map_while(Result::ok) {
            match l.strip_prefix(RESULT_PREFIX) {
                Some(json) => result = serde_json::from_str::<Value>(json).ok(),
                None => log(&l),
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let errors = errors.join().unwrap_or_default();
    for e in &errors {
        log(e);
    }
    match (status.success(), result) {
        (true, Some(r)) => Ok(r),
        _ => Err(errors.last().cloned().unwrap_or_else(|| "instrumentation failed".into())),
    }
}

/// A JavaScript copy with source maps for the debugger (`nodex-ts instrument --debug`):
/// `{entry, out, source}`.
pub fn sidecar_debug_build(root: &Path, out_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<Value, String> {
    sidecar(&["--root".as_ref(), root.as_os_str(), "--out".as_ref(), out_dir.as_os_str(), "--debug".as_ref(), "1".as_ref()], log)
}

impl TraceInstrumenter for NodeInstrumenter {
    fn prepare(&self, root: &Path, out_dir: &Path, env: &RunEnv, log: &mut dyn FnMut(&str)) -> Result<RunCommand, String> {
        std::fs::create_dir_all(out_dir).map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
        log("Instrumenting (TypeScript compiler) …");
        let result = sidecar(
            &[
                "--root".as_ref(),
                root.as_os_str(),
                "--out".as_ref(),
                out_dir.as_os_str(),
                "--target".as_ref(),
                "auto".as_ref(),
                "--ws-url".as_ref(),
                env.ws_url.as_ref(),
            ],
            log,
        )?;
        let field = |k: &str| result.get(k).and_then(Value::as_str).map(str::to_owned).ok_or_else(|| format!("instrumenter reported no {k}"));
        if result.get("mode").and_then(Value::as_str) == Some("browser") {
            let command: Vec<String> = result
                .get("command")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect())
                .unwrap_or_default();
            let (program, args) = command.split_first().ok_or("instrumenter reported no dev command")?;
            return Ok(RunCommand {
                program: program.clone(),
                args: args.to_vec(),
                cwd: PathBuf::from(field("cwd")?),
                trace_map: PathBuf::from(field("map")?),
                description: field("description")?,
                kind: RunKind::DevServer,
                env: Vec::new(),
            });
        }
        Ok(RunCommand {
            program: node().to_string_lossy().into_owned(),
            args: vec!["--require".into(), field("runtime")?, field("entry")?],
            cwd: root.to_path_buf(),
            trace_map: PathBuf::from(field("map")?),
            description: field("description")?,
            kind: RunKind::Program,
            env: Vec::new(),
        })
    }

    fn reinstrument(&self, root: &Path, out_dir: &Path, file: &Path, first_id: i32) -> Option<Result<String, String>> {
        let traced = file.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e, "ts" | "tsx" | "js" | "jsx" | "mts" | "cts" | "mjs" | "cjs" | "vue"));
        if !traced || !out_dir.join("web").join("src").is_dir() {
            return None;
        }
        let first = first_id.to_string();
        let result = sidecar(
            &[
                "--root".as_ref(),
                root.as_os_str(),
                "--out".as_ref(),
                out_dir.as_os_str(),
                "--file".as_ref(),
                file.as_os_str(),
                "--first-id".as_ref(),
                first.as_ref(),
            ],
            &mut |_| {},
        );
        Some(result.map(|r| r.get("methods").cloned().unwrap_or(Value::Array(Vec::new())).to_string()))
    }
}
