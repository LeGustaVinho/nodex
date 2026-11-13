use std::path::{Path, PathBuf};

pub struct Encounter { pub hit_layers: Option<Vec<String>> }

pub fn update_hit_layers(encounter: &mut Encounter, layers: Vec<String>) -> Option<Vec<String>> {
    encounter.hit_layers.replace(layers)
}

pub fn restore_hit_layers(encounter: &mut Encounter, outer: Option<Vec<String>>) -> Vec<String> {
    std::mem::replace(&mut encounter.hit_layers, outer).unwrap_or_default()
}

pub fn replace_attribute(prior: &mut f64, value: f64) -> f64 {
    core::mem::replace(prior, value)
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
