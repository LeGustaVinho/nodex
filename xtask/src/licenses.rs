//! `THIRD-PARTY-NOTICES.txt`: the licenses of everything the installer ships that Nodex did not
//! write — Rust crates in the desktop binary, npm packages bundled into the web UI, the
//! TypeScript compiler of the TypeScript sidecar, and the NuGet packages of the .NET sidecars.
//!
//! License texts come from each package's own files (`LICENSE*`, `COPYING*`, `NOTICE*`);
//! identical texts are printed once with every package that uses them. A package without such
//! a file is listed with its license expression (the standard text is at spdx.org).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::{Result, tool};

/// One shipped component.
struct Component {
    /// `name version`.
    label: String,
    license: String,
    url: String,
    /// Its license and notice files, read in full.
    texts: Vec<String>,
}

/// Writes the notices for the components of `root` into `out`. The .NET and TypeScript parts
/// read the staged sidecars in `stage` (run after staging).
pub fn write(root: &Path, stage: &Path, out: &Path) -> Result {
    let mut sections = Vec::new();
    sections.push(("Rust crates (the Nodex application)", rust(root)?));
    sections.push(("npm packages (the user interface)", npm(root)?));
    sections.push(("TypeScript compiler (the TypeScript helper)", typescript(stage)?));
    sections.push((".NET packages (the C# helpers)", dotnet(stage)?));

    let mut text = String::from(
        "Third-party software in Nodex\n\
         =============================\n\n\
         Nodex is distributed under the MIT License (see LICENSE). It includes the following\n\
         software, distributed under their own licenses, which are reproduced below.\n",
    );
    let mut total = 0;
    for (title, components) in &sections {
        total += components.len();
        text.push_str(&format!("\n\n{}\n{}\n", title, "=".repeat(title.chars().count())));
        render(&mut text, components);
    }
    fs::write(out, text).map_err(|e| format!("cannot write {}: {e}", out.display()))?;
    println!("Third-party notices: {} components in {}", total, out.display());
    Ok(())
}

/// Groups the components by license text and prints each text once.
fn render(out: &mut String, components: &[Component]) {
    out.push_str("\nComponents:\n");
    for c in components {
        out.push_str(&format!("  {}  ({}){}\n", c.label, c.license, if c.url.is_empty() { String::new() } else { format!("  {}", c.url) }));
    }
    let mut by_text: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut without: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for c in components {
        if c.texts.is_empty() {
            without.entry(c.license.clone()).or_default().insert(c.label.clone());
        }
        for t in &c.texts {
            by_text.entry(t.clone()).or_default().insert(c.label.clone());
        }
    }
    for (text, users) in &by_text {
        out.push_str(&format!("\n----------------------------------------------------------------------\nUsed by: {}\n\n", users.iter().cloned().collect::<Vec<_>>().join(", ")));
        out.push_str(text.trim_end());
        out.push('\n');
    }
    for (license, users) in &without {
        out.push_str(&format!(
            "\n----------------------------------------------------------------------\n{} (standard text: https://spdx.org/licenses/), used by: {}\n",
            license,
            users.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
}

/// `LICENSE*`, `COPYING*`, `NOTICE*` (any case) in a package folder.
fn license_texts(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_ascii_lowercase();
            let rtf = name.ends_with(".rtf");
            !rtf && (name.starts_with("licen") || name.starts_with("copying") || name.starts_with("notice") || name.starts_with("thirdpartynotice"))
        })
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|p| fs::read(p).ok())
        .map(|b| String::from_utf8_lossy(&b).replace("\r\n", "\n"))
        .filter(|t| !t.trim().is_empty())
        .collect()
}

fn output(cmd: &mut Command) -> Result<String> {
    let shown = format!("{cmd:?}");
    let out = cmd.output().map_err(|e| format!("cannot run {shown}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{shown} failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The crates in `nodex-desktop`'s normal dependency tree on Windows, from the registry.
fn rust(root: &Path) -> Result<Vec<Component>> {
    let text = output(
        Command::new("cargo")
            .args(["metadata", "--format-version", "1", "--locked", "--filter-platform", "x86_64-pc-windows-msvc"])
            .current_dir(root),
    )?;
    let meta: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let packages: BTreeMap<&str, &Value> =
        meta["packages"].as_array().into_iter().flatten().filter_map(|p| Some((p["id"].as_str()?, p))).collect();
    let nodes: BTreeMap<&str, &Value> =
        meta["resolve"]["nodes"].as_array().into_iter().flatten().filter_map(|n| Some((n["id"].as_str()?, n))).collect();
    let start = packages.values().find(|p| p["name"] == "nodex-desktop").and_then(|p| p["id"].as_str()).ok_or("nodex-desktop not in cargo metadata")?;

    let mut seen = BTreeSet::new();
    let mut stack = vec![start];
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        for dep in nodes.get(id).and_then(|n| n["deps"].as_array()).into_iter().flatten() {
            // Normal dependencies only (`kind: null`): build and dev dependencies are not shipped.
            let normal = dep["dep_kinds"].as_array().into_iter().flatten().any(|k| k["kind"].is_null());
            if normal && let Some(pkg) = dep["pkg"].as_str() {
                stack.push(pkg);
            }
        }
    }
    let mut out: Vec<Component> = seen
        .iter()
        .filter_map(|id| packages.get(id))
        .filter(|p| !p["source"].is_null())
        .map(|p| {
            let dir = Path::new(p["manifest_path"].as_str().unwrap_or("")).parent().map(Path::to_path_buf).unwrap_or_default();
            Component {
                label: format!("{} {}", p["name"].as_str().unwrap_or("?"), p["version"].as_str().unwrap_or("?")),
                license: p["license"].as_str().unwrap_or("see its repository").to_owned(),
                url: p["repository"].as_str().unwrap_or("").to_owned(),
                texts: license_texts(&dir),
            }
        })
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(out)
}

/// The web UI's production dependencies (what Vite bundles).
fn npm(root: &Path) -> Result<Vec<Component>> {
    let text = output(tool("pnpm").args(["--dir", "web", "licenses", "list", "--prod", "--json"]).current_dir(root))?;
    let groups: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (license, packages) in groups.as_object().into_iter().flatten() {
        for p in packages.as_array().into_iter().flatten() {
            let name = p["name"].as_str().unwrap_or("?");
            let versions: Vec<&str> = p["versions"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
            let texts = p["paths"].as_array().into_iter().flatten().filter_map(Value::as_str).next().map(|d| license_texts(Path::new(d))).unwrap_or_default();
            // pnpm says "Unknown" when package.json has no license field; the file may still say.
            let license = if license == "Unknown" && !texts.is_empty() { "see the license text".to_owned() } else { license.clone() };
            out.push(Component { label: format!("{name} {}", versions.join(", ")), license, url: p["homepage"].as_str().unwrap_or("").to_owned(), texts });
        }
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(out)
}

fn typescript(stage: &Path) -> Result<Vec<Component>> {
    let dir = stage.join("sidecars/nodex-ts/node_modules/typescript");
    let package: Value = serde_json::from_str(&fs::read_to_string(dir.join("package.json")).map_err(|e| format!("stage the sidecars first: {e}"))?)
        .map_err(|e| e.to_string())?;
    Ok(vec![Component {
        label: format!("typescript {}", package["version"].as_str().unwrap_or("?")),
        license: package["license"].as_str().unwrap_or("Apache-2.0").to_owned(),
        url: "https://github.com/microsoft/TypeScript".into(),
        texts: license_texts(&dir),
    }])
}

/// NuGet packages listed in the published sidecars' `deps.json`, described by their `.nuspec`.
fn dotnet(stage: &Path) -> Result<Vec<Component>> {
    let nuget = std::env::var_os("NUGET_PACKAGES")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(|h| PathBuf::from(h).join(".nuget/packages")))
        .ok_or("cannot find the NuGet package folder")?;
    let mut found = BTreeSet::new();
    for name in ["Nodex.Roslyn", "Nodex.Weaver"] {
        let path = stage.join("sidecars").join(name).join(format!("{name}.deps.json"));
        let deps: Value = serde_json::from_str(&fs::read_to_string(&path).map_err(|e| format!("stage the sidecars first ({}): {e}", path.display()))?)
            .map_err(|e| e.to_string())?;
        for (key, lib) in deps["libraries"].as_object().into_iter().flatten() {
            if lib["type"] == "package" {
                found.insert(key.clone());
            }
        }
    }
    let mut out = Vec::new();
    for key in found {
        let Some((id, version)) = key.split_once('/') else { continue };
        let dir = nuget.join(id.to_ascii_lowercase()).join(version);
        let nuspec = fs::read_dir(&dir)
            .ok()
            .and_then(|e| e.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "nuspec")))
            .and_then(|p| fs::read_to_string(p).ok())
            .unwrap_or_default();
        let tag = |name: &str| {
            let start = nuspec.find(&format!("<{name}"))?;
            let open_end = nuspec[start..].find('>')? + start + 1;
            let close = nuspec[open_end..].find(&format!("</{name}>"))? + open_end;
            Some(nuspec[open_end..close].trim().to_owned())
        };
        let license = tag("license").or_else(|| tag("licenseUrl")).unwrap_or_else(|| "see its project".into());
        out.push(Component { label: format!("{id} {version}"), license, url: tag("projectUrl").unwrap_or_default(), texts: license_texts(&dir) });
    }
    Ok(out)
}
