//! Debugging Node.js programs with vscode-js-debug (Microsoft, MIT): the project is compiled
//! to JavaScript with source maps (`nodex-ts instrument --debug`) and launched as `pwa-node`;
//! breakpoints and stack frames map back to the TypeScript sources.

use std::path::Path;

use nodex_lang_api::{AdapterDownload, AdapterSpec, AdapterTransport, DebugLaunch, DebugLauncher};
use serde_json::{Value, json};

use crate::semantic::node;

pub struct NodeDebugger;

const DOWNLOADS: &[AdapterDownload] = &[AdapterDownload {
    os: "any",
    arch: "any",
    url: "https://github.com/microsoft/vscode-js-debug/releases/download/v1.140.0/js-debug-dap-v1.140.0.tar.gz",
    sha256: "27dab92937ec1ab35821ae955aac867544fe06a1b6307229049f2d789af10968",
    size: 1_249_709,
    format: "tar.gz",
    program: "js-debug/src/dapDebugServer.js",
}];

impl DebugLauncher for NodeDebugger {
    fn adapter(&self) -> AdapterSpec {
        AdapterSpec {
            name: "js-debug",
            version: "1.140.0",
            adapter_id: "pwa-node",
            homepage: "https://github.com/microsoft/vscode-js-debug",
            downloads: DOWNLOADS,
            env_override: "NODEX_JS_DEBUG",
            interpreter: Some("node"),
            args: &["{port}", "127.0.0.1"],
            transport: AdapterTransport::Tcp,
        }
    }

    fn prepare(&self, root: &Path, out_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<DebugLaunch, String> {
        std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
        if crate::instrument::is_browser_app(root) {
            return Err("debugging browser apps is not supported yet; use Run to watch them live".into());
        }
        log("Compiling with source maps (TypeScript compiler) …");
        let result = crate::instrument::sidecar_debug_build(root, out_dir, log)?;
        let field = |k: &str| result.get(k).and_then(Value::as_str).map(str::to_owned).ok_or_else(|| format!("compiler reported no {k}"));
        let out_js = field("out")?;
        Ok(DebugLaunch {
            launch: json!({
                "name": "Nodex",
                "type": "pwa-node",
                "request": "launch",
                "program": field("entry")?,
                "cwd": root,
                "runtimeExecutable": node(),
                "sourceMaps": true,
                // The compiled copy lives under `.nodex/`: name it explicitly, since `**` globs
                // skip dot-folders.
                "outFiles": [format!("{}/**/*.js", out_js.replace('\\', "/"))],
                "resolveSourceMapLocations": [format!("{}/**", out_js.replace('\\', "/")), "!**/node_modules/**"],
                "pauseForSourceMap": true,
                "skipFiles": ["<node_internals>/**"],
                "console": "internalConsole",
            }),
            description: format!("{} (Node.js, debug build)", field("source")?),
        })
    }
}
