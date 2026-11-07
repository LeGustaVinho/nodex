//! Locates the .NET sidecars shipped in `sidecars/dotnet`.

use std::path::PathBuf;

/// Finds `<name>.dll`: the `env` variable, next to the executable (`sidecars/<name>/`), or a
/// Release/Debug build in the source tree.
pub fn locate(name: &str, env: &str) -> Option<PathBuf> {
    let installed = format!("sidecars/{name}/{name}.dll");
    let release = format!("sidecars/dotnet/{name}/bin/Release/net10.0/{name}.dll");
    let debug = format!("sidecars/dotnet/{name}/bin/Debug/net10.0/{name}.dll");
    nodex_lang_api::sidecar::locate(env, &[&installed], &[&release, &debug])
}
