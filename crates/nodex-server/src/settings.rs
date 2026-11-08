//! User preferences, in `settings.json` in the settings folder ([`crate::projects::config_dir`]).
//! Unknown or missing fields take their defaults, so older and newer files both load.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
#[ts(export)]
pub struct Settings {
    /// The desktop app looks for a new version when it starts.
    pub check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { check_updates: true }
    }
}

fn file() -> Option<std::path::PathBuf> {
    crate::projects::config_dir().map(|d| d.join("settings.json"))
}

pub fn load() -> Settings {
    file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(settings: &Settings) -> Result<(), String> {
    let file = file().ok_or("no settings folder on this system")?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&file, text).map_err(|e| format!("cannot write {}: {e}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_take_defaults() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, Settings::default());
        let s: Settings = serde_json::from_str(r#"{"check_updates": false, "future": 1}"#).unwrap();
        assert!(!s.check_updates);
    }
}
