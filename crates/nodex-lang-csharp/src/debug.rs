//! Debugging .NET programs with netcoredbg (Samsung, MIT): a plain `dotnet build -c Debug`
//! and a `coreclr` launch.

use std::path::Path;

use nodex_lang_api::{AdapterDownload, AdapterSpec, AdapterTransport, DebugLaunch, DebugLauncher};
use serde_json::json;

use crate::instrument::{entry_assembly, find_executable_project, run_logged};

pub struct DotnetDebugger;

const DOWNLOADS: &[AdapterDownload] = &[
    AdapterDownload {
        os: "windows",
        arch: "x86_64",
        url: "https://github.com/Samsung/netcoredbg/releases/download/3.2.0-1092/netcoredbg-win64.zip",
        sha256: "3c410a45fa502415203a94fcb88654af65bf8e3dac158a5527a722e7a6b9274a",
        size: 3_524_161,
        format: "zip",
        program: "netcoredbg/netcoredbg.exe",
    },
    AdapterDownload {
        os: "linux",
        arch: "x86_64",
        url: "https://github.com/Samsung/netcoredbg/releases/download/3.2.0-1092/netcoredbg-linux-amd64.tar.gz",
        sha256: "080eb3b2d2152465f599d3b33d1ee6e747794e11cc0a3773ec689f5e5f2c5afa",
        size: 3_510_717,
        format: "tar.gz",
        program: "netcoredbg/netcoredbg",
    },
    AdapterDownload {
        os: "linux",
        arch: "aarch64",
        url: "https://github.com/Samsung/netcoredbg/releases/download/3.2.0-1092/netcoredbg-linux-arm64.tar.gz",
        sha256: "065ff49badec8a695dbea2de6ab6a330c774a191e426a217ab8cc05250627ccb",
        size: 3_424_271,
        format: "tar.gz",
        program: "netcoredbg/netcoredbg",
    },
    AdapterDownload {
        os: "macos",
        arch: "aarch64",
        url: "https://github.com/Samsung/netcoredbg/releases/download/3.2.0-1092/netcoredbg-osx-arm64.zip",
        sha256: "f4fa33b3ff874910cc184b4bb3b9c56d0abdf5c6521cee0b144d7c6e4a6e59ea",
        size: 3_389_504,
        format: "zip",
        program: "netcoredbg/netcoredbg",
    },
];

impl DebugLauncher for DotnetDebugger {
    fn adapter(&self) -> AdapterSpec {
        AdapterSpec {
            name: "netcoredbg",
            version: "3.2.0-1092",
            adapter_id: "coreclr",
            homepage: "https://github.com/Samsung/netcoredbg",
            downloads: DOWNLOADS,
            env_override: "NODEX_NETCOREDBG",
            interpreter: None,
            args: &["--interpreter=vscode"],
            transport: AdapterTransport::Stdio,
        }
    }

    fn prepare(&self, root: &Path, out_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<DebugLaunch, String> {
        let project = find_executable_project(root)?;
        let build = out_dir.join("debug-build");
        if build.exists() {
            std::fs::remove_dir_all(&build).map_err(|e| format!("cannot clean {}: {e}", build.display()))?;
        }
        log(&format!("Building {} for debugging …", project.file_name().unwrap_or_default().to_string_lossy()));
        let mut cmd = std::process::Command::new("dotnet");
        cmd.arg("build").arg(&project).args(["-c", "Debug", "-nologo", "-v", "minimal", "-o"]).arg(&build);
        run_logged(cmd, log).map_err(|e| format!("build failed: {e}"))?;
        let app = entry_assembly(&build, &project)?;
        let cwd = project.parent().unwrap_or(root);
        Ok(DebugLaunch {
            launch: json!({
                "name": "Nodex",
                "type": "coreclr",
                "request": "launch",
                "program": build.join(format!("{app}.dll")),
                "args": [],
                "cwd": cwd,
                "console": "internalConsole",
                "stopAtEntry": false,
                "justMyCode": true,
            }),
            description: format!("{app} (.NET, debug build)"),
        })
    }
}
