//! Project detection: `package.json` / `tsconfig.json` (workspaces included), or loose
//! TypeScript/JavaScript files.

use std::path::{Path, PathBuf};

use nodex_ir::{Language, PackageRef};
use nodex_lang_api::{ProjectDescriptor, is_ignored_dir};
use serde_json::Value;

const CODE_EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "vue"];

/// Dependencies that describe what kind of project this is.
const FLAVOR_DEPS: &[(&str, &str)] = &[
    ("react", "react"),
    ("electron", "electron"),
    ("vue", "vue"),
    ("@angular/core", "angular"),
    ("next", "next"),
    ("express", "express"),
    ("@types/node", "node"),
];

pub fn detect(root: &Path) -> Option<ProjectDescriptor> {
    let mut manifests = Vec::new();
    find_files(root, 0, 3, &mut |p| {
        matches!(p.file_name().and_then(|n| n.to_str()), Some("package.json" | "tsconfig.json" | "jsconfig.json"))
    }, &mut manifests);
    manifests.sort_by_key(|p| p.components().count());

    if manifests.is_empty() && !has_code_file(root, 0, 4) {
        return None;
    }
    if manifests.iter().all(|m| m.file_name().is_some_and(|n| n == "package.json")) && !has_code_file(root, 0, 6) {
        // A package.json without any TS/JS (tooling only, for example a C# repo with npm scripts).
        return None;
    }

    let mut local_names = Vec::new();
    let mut packages: Vec<PackageRef> = Vec::new();
    let mut root_name = None;
    for m in manifests.iter().filter(|m| m.file_name().is_some_and(|n| n == "package.json")) {
        let Ok(text) = std::fs::read_to_string(m) else { continue };
        let Ok(json) = serde_json::from_str::<Value>(&text) else { continue };
        if let Some(name) = json.get("name").and_then(Value::as_str) {
            local_names.push(name.to_owned());
            if root_name.is_none() {
                root_name = Some(name.to_owned());
            }
        }
        for section in ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"] {
            if let Some(deps) = json.get(section).and_then(Value::as_object) {
                for (name, version) in deps {
                    packages.push(PackageRef { name: name.clone(), version: version.as_str().map(str::to_owned) });
                }
            }
        }
    }
    // Workspace packages are part of the project, not external dependencies.
    packages.retain(|p| !local_names.contains(&p.name));
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    packages.dedup_by(|a, b| a.name == b.name);

    let mut flavors: Vec<String> = FLAVOR_DEPS
        .iter()
        .filter(|(dep, _)| packages.iter().any(|p| p.name == *dep))
        .map(|(_, f)| (*f).to_owned())
        .collect();
    flavors.dedup();

    let name = root_name
        .or_else(|| root.file_name().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "project".into());

    Some(ProjectDescriptor { language: Language::Typescript, name, manifests, packages, flavors })
}

/// Folders detection never enters: ignored ones and hidden ones.
fn skip(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|n| is_ignored_dir(n) || n.starts_with('.'))
}

fn find_files(dir: &Path, depth: usize, max: usize, pred: &mut dyn FnMut(&Path) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        // The entry's type comes with the listing; `path.is_dir()` would stat every file.
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if depth < max && !skip(&entry.file_name()) {
                find_files(&entry.path(), depth + 1, max, pred, out);
            }
        } else {
            let path = entry.path();
            if pred(&path) {
                out.push(path);
            }
        }
    }
}

/// A file name with a code extension, not minified and not a declaration file.
fn is_code_file(name: &str) -> bool {
    Path::new(name).extension().and_then(|e| e.to_str()).is_some_and(|e| CODE_EXTENSIONS.contains(&e))
        && !name.ends_with(".min.js")
        && !name.ends_with(".d.ts")
}

/// True as soon as one code file is found within `max` levels (a folder's files before its
/// subfolders, so a shallow file ends the search early).
fn has_code_file(dir: &Path, depth: usize, max: usize) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        let name = entry.file_name();
        if kind.is_dir() {
            if depth < max && !skip(&name) {
                subdirs.push(entry.path());
            }
        } else if is_code_file(&name.to_string_lossy()) {
            return true;
        }
    }
    subdirs.iter().any(|d| has_code_file(d, depth + 1, max))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_typescript_sample() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ts/Sample");
        let d = detect(&root).expect("sample is a TypeScript project");
        assert_eq!(d.name, "nodex-ts-sample");
        assert!(d.flavors.iter().any(|f| f == "node"));
    }
}
