//! Debugging Rust programs with CodeLLDB (vadimcn, MIT): a plain `cargo build` of the binary
//! (into Nodex's own target folder) and an `lldb` launch.

use std::path::Path;
use std::process::{Command, Stdio};

use nodex_lang_api::{AdapterDownload, AdapterSpec, AdapterTransport, DebugLaunch, DebugLauncher};
use serde_json::json;

pub struct RustDebugger;

const DOWNLOADS: &[AdapterDownload] = &[
    AdapterDownload {
        os: "windows",
        arch: "x86_64",
        url: "https://github.com/vadimcn/codelldb/releases/download/v1.12.3/codelldb-win32-x64.vsix",
        sha256: "a916e509308dac817732f63ca604a8b93ed29cd16f38a2fa9f0b64ed58e8f51a",
        size: 50_943_475,
        format: "zip",
        program: "extension/adapter/codelldb.exe",
    },
    AdapterDownload {
        os: "linux",
        arch: "x86_64",
        url: "https://github.com/vadimcn/codelldb/releases/download/v1.12.3/codelldb-linux-x64.vsix",
        sha256: "1cd7f386598022b51a5b93b9ffa23e812b23f519cfe1833384ec4bef4bfd1be1",
        size: 55_571_094,
        format: "zip",
        program: "extension/adapter/codelldb",
    },
    AdapterDownload {
        os: "linux",
        arch: "aarch64",
        url: "https://github.com/vadimcn/codelldb/releases/download/v1.12.3/codelldb-linux-arm64.vsix",
        sha256: "0887f67d440554617894266f80706b700907c36b95e6e49d23b95a0e05318101",
        size: 54_544_514,
        format: "zip",
        program: "extension/adapter/codelldb",
    },
    AdapterDownload {
        os: "macos",
        arch: "aarch64",
        url: "https://github.com/vadimcn/codelldb/releases/download/v1.12.3/codelldb-darwin-arm64.vsix",
        sha256: "2f114a990e1b368dd1dbd33c80c0e719767af2d228391ec0df0571c957f9ac91",
        size: 45_390_048,
        format: "zip",
        program: "extension/adapter/codelldb",
    },
    AdapterDownload {
        os: "macos",
        arch: "x86_64",
        url: "https://github.com/vadimcn/codelldb/releases/download/v1.12.3/codelldb-darwin-x64.vsix",
        sha256: "e25cc716b94c62c07fec268ff2785d2b797245b160502baef8b9c970a0c4d8e8",
        size: 47_735_596,
        format: "zip",
        program: "extension/adapter/codelldb",
    },
];

impl DebugLauncher for RustDebugger {
    fn adapter(&self) -> AdapterSpec {
        AdapterSpec {
            name: "codelldb",
            version: "1.12.3",
            adapter_id: "lldb",
            homepage: "https://github.com/vadimcn/codelldb",
            downloads: DOWNLOADS,
            env_override: "NODEX_CODELLDB",
            interpreter: None,
            args: &["--port", "{port}"],
            transport: AdapterTransport::Tcp,
        }
    }

    fn prepare(&self, root: &Path, out_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<DebugLaunch, String> {
        let packages = crate::project::cargo_packages(root);
        let (package, (bin, _)) = packages
            .iter()
            .find_map(|p| p.bins.first().map(|b| (p, b.clone())))
            .ok_or("no binary to debug: add src/main.rs or a [[bin]] target")?;
        let target = out_dir.join("debug-target");
        log(&format!("cargo build --bin {bin} …"));
        let mut child = Command::new("cargo")
            .arg("build")
            .arg("--manifest-path")
            .arg(&package.manifest)
            .arg("--bin")
            .arg(&bin)
            .arg("--target-dir")
            .arg(&target)
            .arg("--color")
            .arg("never")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start cargo ({e}); is Rust installed?"))?;
        let mut errors = Vec::new();
        if let Some(stderr) = child.stderr.take() {
            use std::io::BufRead;
            for l in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
                log(&l);
                if l.starts_with("error") {
                    errors.push(l);
                }
            }
        }
        if !child.wait().map_err(|e| e.to_string())?.success() {
            return Err(errors.first().cloned().unwrap_or_else(|| "cargo build failed".into()));
        }
        let exe = target.join("debug").join(format!("{bin}{}", std::env::consts::EXE_SUFFIX));
        Ok(DebugLaunch {
            launch: json!({
                "name": "Nodex",
                "type": "lldb",
                "request": "launch",
                "program": exe,
                "args": [],
                "cwd": root,
                "sourceLanguages": ["rust"],
                "terminal": "console",
            }),
            description: format!("{bin} (Rust, debug build)"),
            attach: false,
        })
    }
}
