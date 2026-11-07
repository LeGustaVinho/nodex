//! Client for the `Nodex.Roslyn` sidecar (`sidecars/dotnet/Nodex.Roslyn`).
//!
//! The sidecar is a .NET process speaking line-delimited JSON over stdin/stdout. It loads the
//! project with MSBuild (or as loose files) and reports semantic facts keyed by byte ranges.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use nodex_ir::semantic::SemanticFacts;
use nodex_lang_api::SemanticProvider;
use serde_json::{Value, json};

const BUILD_HINT: &str = "build it with `dotnet build sidecars/dotnet/Nodex.Roslyn -c Release` or set NODEX_ROSLYN";

pub struct RoslynSidecar {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    description: String,
}

impl RoslynSidecar {
    /// Starts the sidecar and opens the project at `root`. Blocks while MSBuild loads.
    pub fn start(root: &Path) -> Result<Self, String> {
        let entry = locate().ok_or_else(|| format!("Roslyn sidecar not found; {BUILD_HINT}"))?;
        let mut command = if entry.extension().is_some_and(|e| e == "dll") {
            let mut c = Command::new("dotnet");
            c.arg(&entry);
            c
        } else {
            Command::new(&entry)
        };
        let log = std::env::var_os("NODEX_ROSLYN_LOG").is_some();
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(if log { Stdio::inherit() } else { Stdio::null() })
            .spawn()
            .map_err(|e| format!("cannot start the Roslyn sidecar ({e}); is the .NET SDK installed?"))?;
        let stdin = child.stdin.take().ok_or("sidecar stdin unavailable")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("sidecar stdout unavailable")?);
        let mut sidecar = Self { child, stdin, stdout, next_id: 1, description: String::new() };
        let opened = sidecar.request("open", json!({ "root": root.to_string_lossy() }))?;
        sidecar.description = opened
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("Roslyn")
            .to_owned();
        Ok(sidecar)
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let line = json!({ "id": id, "method": method, "params": params }).to_string();
        writeln!(self.stdin, "{line}")
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("Roslyn sidecar stopped ({e})"))?;
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = self.stdout.read_line(&mut buf).map_err(|e| format!("Roslyn sidecar read failed ({e})"))?;
            if read == 0 {
                return Err("Roslyn sidecar exited unexpectedly".into());
            }
            let Ok(message) = serde_json::from_str::<Value>(buf.trim()) else { continue };
            if message.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(err) = message.get("error").and_then(Value::as_str) {
                return Err(format!("Roslyn: {err}"));
            }
            return Ok(message.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    fn facts(&mut self, method: &str, params: Value) -> Result<SemanticFacts, String> {
        let value = self.request(method, params)?;
        serde_json::from_value(value).map_err(|e| format!("unexpected facts from Roslyn sidecar: {e}"))
    }
}

impl SemanticProvider for RoslynSidecar {
    fn describe(&self) -> String {
        self.description.clone()
    }

    fn analyze(&mut self) -> Result<SemanticFacts, String> {
        self.facts("analyze", Value::Null)
    }

    fn update(&mut self, changed: &[PathBuf]) -> Result<SemanticFacts, String> {
        let files: Vec<String> = changed.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        self.facts("update", json!({ "files": files }))
    }
}

impl Drop for RoslynSidecar {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "{}", json!({ "id": 0, "method": "shutdown" }));
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Finds the sidecar: `NODEX_ROSLYN`, next to the executable, or a build in the source tree.
pub fn locate() -> Option<PathBuf> {
    crate::sidecar::locate("Nodex.Roslyn", "NODEX_ROSLYN")
}
