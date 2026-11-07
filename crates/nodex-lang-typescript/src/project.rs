//! Project detection: `package.json` / `tsconfig.json` (workspaces included), or loose
//! TypeScript/JavaScript files.

use std::path::{Path, PathBuf};

use nodex_ir::{Language, PackageRef};
use nodex_lang_api::ProjectDescriptor;
use serde_json::Value;

const SKIP_DIRS: &[&str] = &[
    "node_modules", ".git", "dist", "build", "out", "coverage", ".next", ".nodex", "bin", "obj", "target", "Library",
    "Temp", ".turbo",
];

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

fn skip(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).is_some_and(|n| SKIP_DIRS.contains(&n) || n.starts_with('.'))
}

fn find_files(dir: &Path, depth: usize, max: usize, pred: &mut dyn FnMut(&Path) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < max && !skip(&path) {
                find_files(&path, depth + 1, max, pred, out);
            }
        } else if pred(&path) {
            out.push(path);
        }
    }
}

fn has_code_file(dir: &Path, depth: usize, max: usize) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < max && !skip(&path) && has_code_file(&path, depth + 1, max) {
                return true;
            }
        } else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| CODE_EXTENSIONS.contains(&e))
            && !path.to_string_lossy().ends_with(".min.js")
            && !path.to_string_lossy().ends_with(".d.ts")
        {
            return true;
        }
    }
    false
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
