//! Client for the TypeScript sidecar (`sidecars/node/nodex-ts`).
//!
//! The sidecar is a Node.js process using the TypeScript compiler API. It speaks the same
//! line-delimited JSON protocol as the Roslyn sidecar and reports facts keyed by UTF-8 byte
//! ranges (it converts TypeScript's UTF-16 positions).

use std::path::{Path, PathBuf};
use std::process::Command;

use nodex_ir::semantic::SemanticFacts;
use nodex_lang_api::SemanticProvider;
use nodex_lang_api::sidecar::LineSidecar;
use serde_json::{Value, json};

pub const BUILD_HINT: &str = "install it with `pnpm --dir sidecars/node/nodex-ts install` (needs Node.js) or set NODEX_TS_SIDECAR";

pub struct TsSidecar {
    process: LineSidecar,
    description: String,
}

impl TsSidecar {
    /// Starts the sidecar and loads the project at `root`.
    pub fn start(root: &Path) -> Result<Self, String> {
        let entry = locate().ok_or_else(|| format!("TypeScript sidecar not found; {BUILD_HINT}"))?;
        let mut command = Command::new(node());
        command.arg(&entry);
        let log = std::env::var_os("NODEX_TS_LOG").is_some();
        let mut process = LineSidecar::spawn(command, "TypeScript", log).map_err(|e| format!("{e}; is Node.js installed?"))?;
        let opened = process.request("open", json!({ "root": root.to_string_lossy() }))?;
        let description = opened.get("source").and_then(Value::as_str).unwrap_or("TypeScript").to_owned();
        Ok(Self { process, description })
    }
}

impl SemanticProvider for TsSidecar {
    fn describe(&self) -> String {
        self.description.clone()
    }

    fn analyze(&mut self) -> Result<SemanticFacts, String> {
        self.process.facts("analyze", Value::Null)
    }

    fn update(&mut self, changed: &[PathBuf]) -> Result<SemanticFacts, String> {
        let files: Vec<String> = changed.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        self.process.facts("update", json!({ "files": files }))
    }
}

/// The Node.js executable: `NODEX_NODE` or `node` on the PATH.
pub fn node() -> PathBuf {
    std::env::var_os("NODEX_NODE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("node"))
}

/// Finds the sidecar's `dist/main.js`: `NODEX_TS_SIDECAR`, next to the executable, or a build
/// in the source tree.
pub fn locate() -> Option<PathBuf> {
    nodex_lang_api::sidecar::locate(
        "NODEX_TS_SIDECAR",
        &["sidecars/nodex-ts/dist/main.js"],
        &["sidecars/node/nodex-ts/dist/main.js"],
    )
}
