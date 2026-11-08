//! Client for the `Nodex.Roslyn` sidecar (`sidecars/dotnet/Nodex.Roslyn`).
//!
//! The sidecar is a .NET process speaking line-delimited JSON over stdin/stdout. It loads the
//! project with MSBuild (or as loose files) and reports semantic facts keyed by byte ranges.

use std::path::{Path, PathBuf};

use nodex_ir::semantic::SemanticFacts;
use nodex_lang_api::SemanticProvider;
use nodex_lang_api::sidecar::LineSidecar;
use serde_json::{Value, json};

const BUILD_HINT: &str = "build it with `dotnet build sidecars/dotnet/Nodex.Roslyn -c Release` or set NODEX_ROSLYN";

pub struct RoslynSidecar {
    process: LineSidecar,
    description: String,
}

impl RoslynSidecar {
    /// Starts the sidecar and opens the project at `root`. Blocks while MSBuild loads.
    pub fn start(root: &Path) -> Result<Self, String> {
        let entry = locate().ok_or_else(|| format!("Roslyn sidecar not found; {BUILD_HINT}"))?;
        let command = if entry.extension().is_some_and(|e| e == "dll") {
            let mut c = nodex_lang_api::process::command("dotnet");
            c.arg(&entry);
            c
        } else {
            nodex_lang_api::process::command(&entry)
        };
        let log = std::env::var_os("NODEX_ROSLYN_LOG").is_some();
        let mut process = LineSidecar::spawn(command, "Roslyn", log)
            .map_err(|e| format!("{e}; is the .NET SDK installed?"))?;
        let opened = process.request("open", json!({ "root": root.to_string_lossy() }))?;
        let description = opened.get("source").and_then(Value::as_str).unwrap_or("Roslyn").to_owned();
        Ok(Self { process, description })
    }

    fn facts(&mut self, method: &str, params: Value) -> Result<SemanticFacts, String> {
        self.process.facts(method, params)
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

/// Finds the sidecar: `NODEX_ROSLYN`, next to the executable, or a build in the source tree.
pub fn locate() -> Option<PathBuf> {
    crate::sidecar::locate("Nodex.Roslyn", "NODEX_ROSLYN")
}
