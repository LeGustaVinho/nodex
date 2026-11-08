//! Project detection: `Cargo.toml` (packages and workspaces).

use std::path::{Path, PathBuf};

use nodex_ir::{Language, PackageRef};
use nodex_lang_api::{ProjectDescriptor, is_ignored_dir};


/// Dependencies that describe what kind of project this is.
const FLAVOR_DEPS: &[(&str, &str)] = &[
    ("tokio", "tokio"),
    ("async-std", "async-std"),
    ("axum", "axum"),
    ("actix-web", "actix"),
    ("bevy", "bevy"),
    ("serde", "serde"),
    ("clap", "clap"),
    ("rayon", "rayon"),
    ("tauri", "tauri"),
];

/// One `Cargo.toml` package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoPackage {
    pub manifest: PathBuf,
    pub name: String,
    pub edition: Option<String>,
    /// Binary targets: `(name, path relative to the package)`.
    pub bins: Vec<(String, String)>,
    pub lib: bool,
    pub dependencies: Vec<PackageRef>,
}

pub fn detect(root: &Path) -> Option<ProjectDescriptor> {
    let packages = cargo_packages(root);
    if packages.is_empty() && !has_rust_file(root, 0, 4) {
        return None;
    }
    let local: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
    let mut deps: Vec<PackageRef> = packages
        .iter()
        .flat_map(|p| p.dependencies.clone())
        .filter(|d| !local.iter().any(|l| l == &d.name || l.replace('-', "_") == d.name.replace('-', "_")))
        .collect();
    deps.sort_by(|a, b| a.name.cmp(&b.name));
    deps.dedup_by(|a, b| a.name == b.name);

    let mut flavors = vec!["rust".to_owned()];
    if packages.iter().any(|p| !p.bins.is_empty()) {
        flavors.push("bin".into());
    }
    if packages.iter().any(|p| p.lib) {
        flavors.push("lib".into());
    }
    if packages.len() > 1 {
        flavors.push("workspace".into());
    }
    if let Some(e) = packages.iter().find_map(|p| p.edition.clone()) {
        flavors.push(format!("edition-{e}"));
    }
    for (dep, f) in FLAVOR_DEPS {
        if deps.iter().any(|d| d.name == *dep) {
            flavors.push((*f).to_owned());
        }
    }
    let name = packages
        .first()
        .map(|p| p.name.clone())
        .or_else(|| root.file_name().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "crate".into());
    let manifests = packages.iter().map(|p| p.manifest.clone()).collect();
    Some(ProjectDescriptor { language: Language::Rust, name, manifests, packages: deps, flavors })
}

/// Every package under `root`: the root manifest's package, its workspace members, and other
/// manifests found within a few levels (shortest path first).
pub fn cargo_packages(root: &Path) -> Vec<CargoPackage> {
    let mut manifests = Vec::new();
    find_manifests(root, 0, 4, &mut manifests);
    manifests.sort_by_key(|p| (p.components().count(), p.clone()));
    manifests.into_iter().filter_map(|m| read_package(&m)).collect()
}

fn read_package(manifest: &Path) -> Option<CargoPackage> {
    let text = std::fs::read_to_string(manifest).ok()?;
    let doc: toml::Table = toml::from_str(&text).ok()?;
    let package = doc.get("package")?.as_table()?;
    let name = package.get("name")?.as_str()?.to_owned();
    let edition = match package.get("edition") {
        Some(toml::Value::String(e)) => Some(e.clone()),
        _ => None,
    };
    let dir = manifest.parent()?;
    let mut bins = Vec::new();
    if let Some(list) = doc.get("bin").and_then(|b| b.as_array()) {
        for b in list.iter().filter_map(|b| b.as_table()) {
            let bin_name = b.get("name").and_then(|n| n.as_str()).unwrap_or(&name).to_owned();
            let path = b.get("path").and_then(|p| p.as_str()).map(str::to_owned).unwrap_or_else(|| format!("src/bin/{bin_name}.rs"));
            bins.push((bin_name, path));
        }
    }
    if bins.is_empty() && dir.join("src/main.rs").is_file() {
        bins.push((name.clone(), "src/main.rs".into()));
    }
    let lib = doc.get("lib").is_some() || dir.join("src/lib.rs").is_file();
    let mut dependencies = Vec::new();
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(t) = doc.get(section).and_then(|d| d.as_table()) {
            for (dep, spec) in t {
                let (real, version) = match spec {
                    toml::Value::String(v) => (dep.clone(), Some(v.clone())),
                    toml::Value::Table(t) => (
                        t.get("package").and_then(|p| p.as_str()).unwrap_or(dep).to_owned(),
                        t.get("version").and_then(|v| v.as_str()).map(str::to_owned),
                    ),
                    _ => (dep.clone(), None),
                };
                dependencies.push(PackageRef { name: real, version });
            }
        }
    }
    Some(CargoPackage { manifest: manifest.to_owned(), name, edition, bins, lib, dependencies })
}

/// Folders detection never enters: ignored ones and hidden ones.
fn skip(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|n| is_ignored_dir(n) || n.starts_with('.'))
}

fn find_manifests(dir: &Path, depth: usize, max: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        // The entry's type comes with the listing; `path.is_dir()` would stat every file.
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if depth < max && !skip(&entry.file_name()) {
                find_manifests(&entry.path(), depth + 1, max, out);
            }
        } else if entry.file_name() == "Cargo.toml" {
            out.push(entry.path());
        }
    }
}

/// True as soon as one `.rs` file is found within `max` levels (a folder's files before its
/// subfolders, so a shallow file ends the search early).
fn has_rust_file(dir: &Path, depth: usize, max: usize) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        let name = entry.file_name();
        if kind.is_dir() {
            if depth < max && !skip(&name) {
                subdirs.push(entry.path());
            }
        } else if Path::new(&name).extension().is_some_and(|e| e == "rs") {
            return true;
        }
    }
    subdirs.iter().any(|d| has_rust_file(d, depth + 1, max))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_this_workspace() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let d = detect(&root).expect("Nodex is a Rust workspace");
        assert!(d.flavors.iter().any(|f| f == "workspace"), "{:?}", d.flavors);
        assert!(d.packages.iter().any(|p| p.name == "tree-sitter"));
        assert!(!d.packages.iter().any(|p| p.name == "nodex-ir"), "workspace crates are not dependencies");
    }
}
