//! Live-trace preparation for Node.js: `nodex-ts instrument` writes an instrumented JavaScript
//! copy of the project and the trace map; the program then runs with the trace runtime
//! preloaded (`node --require nodex-trace.cjs <entry>`).

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use nodex_lang_api::{RunCommand, TraceInstrumenter};
use serde_json::Value;

use crate::semantic::{BUILD_HINT, locate, node};

const RESULT_PREFIX: &str = "NODEX-RESULT ";

pub struct NodeInstrumenter;

impl TraceInstrumenter for NodeInstrumenter {
    fn prepare(&self, root: &Path, out_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<RunCommand, String> {
        let sidecar = locate().ok_or_else(|| format!("TypeScript sidecar not found; {BUILD_HINT}"))?;
        std::fs::create_dir_all(out_dir).map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
        log("Instrumenting (TypeScript compiler) …");
        let mut child = Command::new(node())
            .arg(&sidecar)
            .arg("instrument")
            .arg("--root")
            .arg(root)
            .arg("--out")
            .arg(out_dir)
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
        let result = match (status.success(), result) {
            (true, Some(r)) => r,
            _ => return Err(errors.last().cloned().unwrap_or_else(|| "instrumentation failed".into())),
        };
        let field = |k: &str| result.get(k).and_then(Value::as_str).map(str::to_owned).ok_or_else(|| format!("instrumenter reported no {k}"));
        Ok(RunCommand {
            program: node().to_string_lossy().into_owned(),
            args: vec!["--require".into(), field("runtime")?, field("entry")?],
            cwd: root.to_path_buf(),
            trace_map: PathBuf::from(field("map")?),
            description: field("description")?,
        })
    }
}
