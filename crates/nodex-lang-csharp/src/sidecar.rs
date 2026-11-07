//! Locates the .NET sidecars shipped in `sidecars/dotnet`.

use std::path::{Path, PathBuf};

/// Finds `<name>.dll`: the `env` variable, next to the executable (`sidecars/<name>/`), or a
/// Release/Debug build in the source tree.
pub fn locate(name: &str, env: &str) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(env).map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let dll = format!("{name}.dll");
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    if let Some(dir) = &exe_dir {
        let installed = dir.join("sidecars").join(name).join(&dll);
        if installed.is_file() {
            return Some(installed);
        }
    }
    let source_tree = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for start in exe_dir.into_iter().chain(std::iter::once(source_tree)) {
        for dir in start.ancestors() {
            for config in ["Release", "Debug"] {
                let candidate = dir
                    .join("sidecars/dotnet")
                    .join(name)
                    .join("bin")
                    .join(config)
                    .join("net10.0")
                    .join(&dll);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}
