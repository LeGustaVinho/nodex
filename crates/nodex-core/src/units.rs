//! Build units of a project: the `.csproj` projects, `package.json` packages and Cargo crates
//! its manifests declare, with the references between them (`<ProjectReference>`,
//! `workspace:`/`file:` dependencies, `path =` dependencies). Component diagrams start from them.

use std::path::Path;

use nodex_ir::{Language, PackageRef};
use serde::Serialize;

/// One project, package or crate.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct BuildUnit {
    /// Display name: the `.csproj` file name, the `package.json` name, the crate name.
    pub name: String,
    pub language: Language,
    /// Project-relative folder of the manifest (`""` for the project root).
    pub root: String,
    /// Project-relative manifest path.
    pub manifest: String,
    /// Roots of the other units this one references in its manifest.
    pub refs: Vec<String>,
    /// External packages it depends on.
    pub packages: Vec<PackageRef>,
}

/// Reads the build units of `manifests` (absolute paths under `root`).
pub fn read_units(root: &Path, manifests: &[(Language, std::path::PathBuf, Vec<PackageRef>)]) -> Vec<BuildUnit> {
    let rel = |p: &Path| -> Option<String> {
        let r = p.strip_prefix(root).ok()?;
        Some(r.to_string_lossy().replace('\\', "/"))
    };
    let mut units: Vec<BuildUnit> = Vec::new();
    // Raw references: a folder (resolved path) or `name:<package name>`.
    let mut raw: Vec<Vec<String>> = Vec::new();
    for (language, manifest, packages) in manifests {
        let file = manifest.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let is_unit = file.ends_with(".csproj") || file == "package.json" || file == "Cargo.toml";
        if !is_unit {
            continue;
        }
        let Some(manifest_rel) = rel(manifest) else { continue };
        let dir = manifest.parent().map(Path::to_path_buf).unwrap_or_default();
        let root_rel = rel(&dir).unwrap_or_default();
        if units.iter().any(|u| u.manifest == manifest_rel) {
            continue;
        }
        let text = std::fs::read_to_string(manifest).unwrap_or_default();
        let folder = dir.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_else(|| "project".into());
        let (name, refs) = if file.ends_with(".csproj") {
            (file.trim_end_matches(".csproj").to_owned(), csproj_refs(&text, &dir, &rel))
        } else if file == "package.json" {
            package_json(&text, &dir, &rel)
        } else {
            cargo(&text, &dir, &rel)
        };
        let name = if name.is_empty() { folder } else { name };
        units.push(BuildUnit { name, language: *language, root: root_rel, manifest: manifest_rel, refs: Vec::new(), packages: packages.clone() });
        raw.push(refs);
    }
    // A Cargo workspace root without a package of its own is not a unit.
    let by_name: Vec<(String, String)> = units.iter().map(|u| (u.name.clone(), u.root.clone())).collect();
    for (unit, refs) in units.iter_mut().zip(raw) {
        for r in refs {
            let target = match r.strip_prefix("name:") {
                Some(name) => by_name.iter().find(|(n, _)| n == name).map(|(_, root)| root.clone()),
                None => Some(r),
            };
            if let Some(t) = target
                && t != unit.root
                && by_name.iter().any(|(_, root)| *root == t)
                && !unit.refs.contains(&t)
            {
                unit.refs.push(t);
            }
        }
        unit.refs.sort();
    }
    units.sort_by(|a, b| a.root.cmp(&b.root).then(a.manifest.cmp(&b.manifest)));
    units
}

/// `<ProjectReference Include="..\Shared\Shared.csproj" />` → the referenced project's folder.
fn csproj_refs(text: &str, dir: &Path, rel: &dyn Fn(&Path) -> Option<String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("ProjectReference") {
        rest = &rest[at + "ProjectReference".len()..];
        let Some(inc) = rest.find("Include=\"") else { break };
        let after = &rest[inc + "Include=\"".len()..];
        let Some(end) = after.find('"') else { break };
        let path = after[..end].replace('\\', "/");
        if let Some(parent) = normalize(&dir.join(&path)).parent()
            && let Some(r) = rel(parent)
        {
            out.push(r);
        }
    }
    out
}

/// The package name and its `workspace:`/`file:`/`link:` dependencies.
fn package_json(text: &str, dir: &Path, rel: &dyn Fn(&Path) -> Option<String>) -> (String, Vec<String>) {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else { return (String::new(), Vec::new()) };
    let name = json.get("name").and_then(|n| n.as_str()).unwrap_or_default().to_owned();
    let mut refs = Vec::new();
    for section in ["dependencies", "devDependencies", "peerDependencies"] {
        let Some(deps) = json.get(section).and_then(|d| d.as_object()) else { continue };
        for (dep, version) in deps {
            let v = version.as_str().unwrap_or_default();
            if let Some(path) = v.strip_prefix("file:").or_else(|| v.strip_prefix("link:")) {
                if let Some(r) = rel(&normalize(&dir.join(path))) {
                    refs.push(r);
                }
            } else if v.starts_with("workspace:") {
                refs.push(format!("name:{dep}"));
            }
        }
    }
    (name, refs)
}

/// The crate name and its `path = "..."` dependencies.
fn cargo(text: &str, dir: &Path, rel: &dyn Fn(&Path) -> Option<String>) -> (String, Vec<String>) {
    let Ok(toml) = text.parse::<toml::Table>() else { return (String::new(), Vec::new()) };
    let name = toml.get("package").and_then(|p| p.get("name")).and_then(|n| n.as_str()).unwrap_or_default().to_owned();
    let mut refs = Vec::new();
    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let Some(deps) = toml.get(section).and_then(|d| d.as_table()) else { continue };
        for (_, spec) in deps {
            if let Some(path) = spec.get("path").and_then(|p| p.as_str())
                && let Some(r) = rel(&normalize(&dir.join(path)))
            {
                refs.push(r);
            }
        }
    }
    (name, refs)
}

/// Resolves `..` and `.` without touching the file system.
fn normalize(path: &Path) -> std::path::PathBuf {
    let mut out = std::path::PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
