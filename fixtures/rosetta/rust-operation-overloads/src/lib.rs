use std::path::{Path, PathBuf};

pub struct Encounter { pub hit_layers: Option<Vec<String>> }

pub fn update_hit_layers(encounter: &mut Encounter, layers: Vec<String>) -> Option<Vec<String>> {
    encounter.hit_layers.replace(layers)
}

pub fn market_groups(data: &str) -> PathBuf {
    Path::new(data).join("market-groups.json")
}

pub fn replace_text(text: &str) -> String {
    text.replace('_', " ")
}

pub fn join_text(parts: &[&str]) -> String {
    parts.join(", ")
}
