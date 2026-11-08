//! The Unity card: the Nodex package in the project (Link, Embed, Update, Remove), the Editor
//! that has it open, and whether scripts are compiled with trace probes.

use std::path::Path;

use nodex_lang_csharp::unity_editor::{self, PackageState};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct UnityEditorInfo {
    pub pid: u32,
    pub version: String,
    /// The Nodex bridge answers (the package is loaded in this Editor).
    pub bridge: bool,
    pub playing: bool,
    pub compiling: bool,
    /// `Debug` or `Release` (attaching a debugger needs `Debug`).
    pub code_optimization: Option<String>,
    /// The loaded scripts carry trace probes.
    pub trace_compiled: bool,
}

#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct UnityStatus {
    /// `missing`, `linked`, `embedded` or `outdated` (an embedded copy older than Nodex's).
    pub package: String,
    /// The installed copy's version, or the `file:` target of a link.
    pub installed: Option<String>,
    /// The version Nodex ships.
    pub bundled: Option<String>,
    pub editor: Option<UnityEditorInfo>,
    /// Nodex asked for scripts with trace probes (`Library/Nodex/trace-on`).
    pub trace_requested: bool,
}

pub fn status(root: &Path) -> Option<UnityStatus> {
    if !unity_editor::is_unity(root) {
        return None;
    }
    let bundled = unity_editor::bundled_package().and_then(|p| unity_editor::package_version(&p));
    let (package, installed) = match unity_editor::package_state(root) {
        PackageState::Missing => ("missing", None),
        PackageState::Linked(target) => ("linked", Some(target)),
        PackageState::Embedded(v) => {
            let outdated = bundled.as_deref().is_some_and(|b| newer(b, &v));
            (if outdated { "outdated" } else { "embedded" }, Some(v))
        }
    };
    let editor = unity_editor::editor(root).map(|e| {
        let b = unity_editor::bridge(root).filter(|b| b.pid == e.process_id);
        UnityEditorInfo {
            pid: e.process_id,
            version: e.version,
            bridge: b.is_some(),
            playing: b.as_ref().is_some_and(|b| b.playing),
            compiling: b.as_ref().is_some_and(|b| b.compiling),
            code_optimization: b.as_ref().map(|b| b.code_optimization.clone()),
            trace_compiled: b.as_ref().is_some_and(|b| b.trace_compiled),
        }
    });
    Some(UnityStatus { package: package.into(), installed, bundled, editor, trace_requested: unity_editor::trace_requested(root) })
}

/// `a` is a newer `x.y.z` version than `b`.
fn newer(a: &str, b: &str) -> bool {
    let parse = |v: &str| v.split('.').map(|p| p.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    parse(a) > parse(b)
}

#[derive(Deserialize, Debug, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum PackageAction {
    Link,
    Embed,
    Update,
    Remove,
}

pub fn package(root: &Path, action: PackageAction) -> Result<(), String> {
    if !unity_editor::is_unity(root) {
        return Err("this is not a Unity project".into());
    }
    match action {
        PackageAction::Link => unity_editor::link_package(root),
        PackageAction::Embed | PackageAction::Update => unity_editor::embed_package(root),
        PackageAction::Remove => unity_editor::remove_package(root),
    }
}

/// Scripts without trace probes again (the bridge recompiles them).
pub fn trace_off(root: &Path) -> Result<(), String> {
    unity_editor::set_trace(root, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(newer("0.2.0", "0.1.9"));
        assert!(newer("0.10.0", "0.9.0"));
        assert!(!newer("0.1.0", "0.1.0"));
    }
}
