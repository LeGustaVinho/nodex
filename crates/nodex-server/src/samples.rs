//! Sample projects shipped with Nodex, opened from the welcome screen.
//!
//! The installed app keeps them in its resource folder (`samples/<id>`); a build in the
//! repository uses the fixtures. Opening one copies it to the per-user data folder first, so its
//! `.nodex/` state and any edits never touch the installation.

use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

/// A sample the welcome screen offers.
#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct SampleInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

struct Sample {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    /// Folder in the repository.
    source: &'static str,
    /// A file that must exist in the folder.
    marker: &'static str,
}

const SAMPLES: &[Sample] = &[
    Sample {
        id: "orders",
        name: "Orders (C#)",
        description: "A small .NET app with services, a repository, records, events and async code. ▶ Run it to watch live traces.",
        source: "fixtures/csharp/Sample",
        marker: "Sample.csproj",
    },
    Sample {
        id: "rosetta",
        name: "One algorithm, three languages",
        description: "The same filter-map-sum in C#, TypeScript and Rust: open each Blueprint and compare them node for node.",
        source: "fixtures/rosetta/filter-map-sum",
        marker: "filter_map_sum.ts",
    },
];

/// Folders never copied (build output and local state).
const SKIP: &[&str] = &["bin", "obj", ".nodex", "target", "node_modules"];

fn bundled(sample: &Sample) -> Option<PathBuf> {
    let installed = format!("samples/{}/{}", sample.id, sample.marker);
    let source = format!("{}/{}", sample.source, sample.marker);
    let env = format!("NODEX_SAMPLE_{}", sample.id.to_uppercase());
    nodex_lang_api::sidecar::locate(&env, &[&installed], &[&source]).and_then(|p| p.parent().map(Path::to_path_buf))
}

/// The samples found in this installation (or repository).
pub fn list() -> Vec<SampleInfo> {
    SAMPLES
        .iter()
        .filter(|s| bundled(s).is_some())
        .map(|s| SampleInfo { id: s.id.into(), name: s.name.into(), description: s.description.into() })
        .collect()
}

/// Copies sample `id` to `<data dir>/samples/<id>` (once) and returns that folder.
pub fn prepare(id: &str) -> Result<PathBuf, String> {
    let sample = SAMPLES.iter().find(|s| s.id == id).ok_or_else(|| format!("unknown sample {id}"))?;
    let source = bundled(sample).ok_or_else(|| format!("the {} sample is missing from this installation", sample.name))?;
    let target = crate::projects::data_dir().join("samples").join(sample.id);
    if !target.join(sample.marker).is_file() {
        copy_dir(&source, &target).map_err(|e| format!("cannot copy the sample to {}: {e}", target.display()))?;
    }
    Ok(target)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if entry.file_type()?.is_dir() {
            if !SKIP.iter().any(|s| name == *s) {
                copy_dir(&entry.path(), &to.join(&name))?;
            }
        } else if !name.to_string_lossy().starts_with("expected.") {
            std::fs::copy(entry.path(), to.join(&name))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_found_in_the_repository_and_copied() {
        let found = list();
        assert!(found.iter().any(|s| s.id == "orders"), "{found:?}");
        assert!(found.iter().any(|s| s.id == "rosetta"), "{found:?}");
        let dir = std::env::temp_dir().join("nodex-sample-copy-test");
        let _ = std::fs::remove_dir_all(&dir);
        let source = bundled(&SAMPLES[1]).unwrap();
        copy_dir(&source, &dir).unwrap();
        assert!(dir.join("filter_map_sum.rs").is_file());
        assert!(!dir.join("expected.ops").exists(), "test expectations are left out");
        assert!(prepare("nope").is_err());
    }
}
