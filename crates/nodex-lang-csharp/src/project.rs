//! Project detection: `.csproj`/`.sln` for .NET, `ProjectSettings` + `Packages/manifest.json` for Unity.

use std::path::{Path, PathBuf};

use nodex_ir::{Language, PackageRef};
use nodex_lang_api::ProjectDescriptor;

const SKIP_DIRS: &[&str] = &["bin", "obj", ".git", "node_modules", "Library", "Temp", "Logs", ".nodex", "target"];

/// `productName` in `ProjectSettings/ProjectSettings.asset` (Player settings).
fn unity_product_name(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("ProjectSettings").join("ProjectSettings.asset")).ok()?;
    let name = text.lines().find_map(|l| l.trim_start().strip_prefix("productName:"))?.trim();
    (!name.is_empty()).then(|| name.trim_matches('"').to_owned())
}

pub fn detect(root: &Path) -> Option<ProjectDescriptor> {
    let unity_manifest = root.join("Packages").join("manifest.json");
    let is_unity = root.join("ProjectSettings").is_dir() || (root.join("Assets").is_dir() && unity_manifest.is_file());

    let mut manifests = Vec::new();
    find_files(root, 0, 3, &mut |p| {
        matches!(p.extension().and_then(|e| e.to_str()), Some("csproj") | Some("sln") | Some("slnx"))
    }, &mut manifests);

    let has_cs = is_unity || !manifests.is_empty() || has_cs_file(root, 0, 4);
    if !has_cs {
        return None;
    }

    let mut packages = Vec::new();
    for m in manifests.iter().filter(|m| m.extension().is_some_and(|e| e == "csproj")) {
        if let Ok(text) = std::fs::read_to_string(m) {
            packages.extend(package_references(&text));
        }
    }
    if is_unity {
        if let Ok(text) = std::fs::read_to_string(&unity_manifest) {
            packages.extend(unity_packages(&text));
        }
        manifests.push(unity_manifest);
    }
    packages.sort_by(|a, b| a.name.cmp(&b.name));
    packages.dedup_by(|a, b| a.name == b.name);

    // Unity generates `.csproj` files named after assemblies (Assembly-CSharp-Editor): use the
    // product name, or the folder's.
    let unity_name = is_unity.then(|| unity_product_name(root).or_else(|| root.file_name().map(|s| s.to_string_lossy().into_owned()))).flatten();
    let name = unity_name.into_iter().chain(manifests
        .iter()
        .find(|m| m.extension().is_some_and(|e| e == "csproj" || e == "sln"))
        .and_then(|m| m.file_stem())
        .map(|s| s.to_string_lossy().into_owned()))
        .next()
        .or_else(|| root.file_name().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "project".into());

    Some(ProjectDescriptor {
        language: Language::Csharp,
        name,
        manifests,
        packages,
        flavors: if is_unity { vec!["unity".into()] } else { Vec::new() },
    })
}

fn find_files(dir: &Path, depth: usize, max: usize, pred: &mut dyn FnMut(&Path) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skip = path.file_name().and_then(|n| n.to_str()).is_some_and(|n| SKIP_DIRS.contains(&n) || n.starts_with('.'));
            if depth < max && !skip {
                find_files(&path, depth + 1, max, pred, out);
            }
        } else if pred(&path) {
            out.push(path);
        }
    }
}

fn has_cs_file(dir: &Path, depth: usize, max: usize) -> bool {
    let mut found = Vec::new();
    find_files(dir, depth, max, &mut |p| p.extension().is_some_and(|e| e == "cs"), &mut found);
    !found.is_empty()
}

/// Extracts `<PackageReference Include="X" Version="Y" />` entries without a full XML parser.
pub fn package_references(csproj: &str) -> Vec<PackageRef> {
    let mut out = Vec::new();
    for chunk in csproj.split("<PackageReference").skip(1) {
        let tag = chunk.split('>').next().unwrap_or("");
        let Some(name) = attr(tag, "Include") else { continue };
        out.push(PackageRef { name, version: attr(tag, "Version") });
    }
    out
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_owned())
}

/// Reads `dependencies` from a Unity `Packages/manifest.json`.
pub fn unity_packages(manifest: &str) -> Vec<PackageRef> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(manifest) else {
        return Vec::new();
    };
    value
        .get("dependencies")
        .and_then(|d| d.as_object())
        .map(|deps| {
            deps.iter()
                .map(|(name, v)| PackageRef { name: name.clone(), version: v.as_str().map(str::to_owned) })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_references() {
        let refs = package_references(r#"<ItemGroup><PackageReference Include="System.Text.Json" Version="10.0.0" /></ItemGroup>"#);
        assert_eq!(refs, vec![PackageRef { name: "System.Text.Json".into(), version: Some("10.0.0".into()) }]);
    }

    #[test]
    fn parses_unity_manifest() {
        let refs = unity_packages(r#"{"dependencies":{"com.unity.inputsystem":"1.11.0"}}"#);
        assert_eq!(refs[0].name, "com.unity.inputsystem");
    }
}
