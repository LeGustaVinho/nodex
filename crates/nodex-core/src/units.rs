//! Build units of a project: the `.csproj` projects, `package.json` packages and Cargo crates
//! its manifests declare, with the references between them (`<ProjectReference>`,
//! `workspace:`/`file:` dependencies, `path =` dependencies). Component diagrams start from them.

use std::collections::{BTreeMap, BTreeSet};
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
    /// Cargo binary targets keyed by project-relative entry file.
    #[serde(skip)]
    pub rust_binaries: BTreeMap<String, String>,
    /// Project-relative scripts passed directly to a Node-compatible launcher.
    #[serde(skip)]
    pub node_entries: BTreeSet<String>,
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
        let mut rust_binaries = BTreeMap::new();
        let mut node_entries = BTreeSet::new();
        let (name, refs) = if file.ends_with(".csproj") {
            (file.trim_end_matches(".csproj").to_owned(), csproj_refs(&text, &dir, &rel))
        } else if file == "package.json" {
            let json = serde_json::from_str::<serde_json::Value>(&text).unwrap_or_default();
            node_entries = node_launch_paths(&json, &dir, &rel);
            package_json(&json, &dir, &rel)
        } else {
            let table = text.parse::<toml::Table>().unwrap_or_default();
            let (name, refs) = cargo(&table, &dir, &rel);
            rust_binaries = cargo_binaries(&table, &name, &dir, &rel);
            (name, refs)
        };
        let name = if name.is_empty() { folder } else { name };
        units.push(BuildUnit { name, language: *language, root: root_rel, manifest: manifest_rel, refs: Vec::new(), packages: packages.clone(), rust_binaries, node_entries });
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
fn package_json(json: &serde_json::Value, dir: &Path, rel: &dyn Fn(&Path) -> Option<String>) -> (String, Vec<String>) {
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

fn node_launch_paths(json: &serde_json::Value, dir: &Path, rel: &dyn Fn(&Path) -> Option<String>) -> BTreeSet<String> {
    json.get("scripts").and_then(serde_json::Value::as_object).into_iter()
        .flat_map(|scripts| scripts.values()).filter_map(|script| {
            let mut words = script.as_str()?.split_whitespace();
            if !matches!(words.next()?, "node" | "tsx" | "ts-node") {
                return None;
            }
            let entry = words.next()?.trim_matches(['\'', '"']);
            let path = Path::new(entry);
            if entry.starts_with('-') || entry.contains(['$', '%'])
                || !matches!(path.extension()?.to_str()?, "js" | "mjs" | "cjs" | "ts" | "mts" | "cts" | "tsx")
            {
                return None;
            }
            rel(&normalize(&dir.join(path)))
        }).collect()
}

/// The crate name and its `path = "..."` dependencies.
fn cargo(toml: &toml::Table, dir: &Path, rel: &dyn Fn(&Path) -> Option<String>) -> (String, Vec<String>) {
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

fn cargo_binaries(
    manifest: &toml::Table,
    package_name: &str,
    dir: &Path,
    rel: &dyn Fn(&Path) -> Option<String>,
) -> BTreeMap<String, String> {
    let Some(package) = manifest.get("package").filter(|_| !package_name.is_empty()) else {
        return BTreeMap::new();
    };
    let declared = manifest.get("bin").and_then(toml::Value::as_array);
    let legacy_edition = package.get("edition").is_none_or(|edition| edition.as_str() == Some("2015"));
    let auto = package.get("autobins").and_then(toml::Value::as_bool)
        .unwrap_or(!legacy_edition || declared.is_none_or(Vec::is_empty));
    let mut binaries = BTreeMap::new();
    let mut names = BTreeSet::new();
    for bin in declared.into_iter().flatten() {
        let Some(name) = bin.get("name").and_then(toml::Value::as_str) else { continue };
        names.insert(name.to_owned());
        let path = bin.get("path").and_then(toml::Value::as_str).map(|path| normalize(&dir.join(path)));
        let candidates = [
            dir.join("src/main.rs"),
            dir.join(format!("src/bin/{name}.rs")),
            dir.join(format!("src/bin/{name}/main.rs")),
        ];
        if let Some(file) = path.or_else(|| {
            candidates.into_iter().skip(usize::from(name != package_name)).find(|p| p.is_file())
        }).and_then(|p| rel(&p)) {
            binaries.insert(file, name.to_owned());
        }
    }
    if auto {
        let main = dir.join("src/main.rs");
        if !names.contains(package_name) && main.is_file()
            && let Some(file) = rel(&main)
        {
            binaries.entry(file).or_insert_with(|| package_name.to_owned());
        }
        for entry in std::fs::read_dir(dir.join("src/bin")).into_iter().flatten().flatten() {
            let path = entry.path();
            let (file, name) = if path.is_dir() {
                (path.join("main.rs"), entry.file_name())
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                (path.clone(), path.file_stem().unwrap_or_default().to_owned())
            } else {
                continue;
            };
            if let Some(name) = name.to_str().filter(|name| !names.contains(*name) && !name.starts_with('.'))
                && file.is_file()
                && let Some(file) = rel(&file)
            {
                binaries.entry(file).or_insert_with(|| name.to_owned());
            }
        }
    }
    binaries
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
