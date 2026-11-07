//! The instrumenter's map file (`nodex-trace-map.json`).

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct PointMap {
    pub file: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MethodMap {
    pub id: i32,
    /// `Namespace.Type::Method`.
    pub name: String,
    /// `method`, `constructor`, or `resume` (async/iterator state machine step).
    pub kind: String,
    pub points: Vec<PointMap>,
    /// Local slot → name (JSON object keys are strings).
    #[serde(default)]
    pub locals: HashMap<String, String>,
    #[serde(default)]
    pub params: Vec<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct TraceMap {
    pub version: u32,
    pub methods: Vec<MethodMap>,
}

impl TraceMap {
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("invalid trace map: {e}"))
    }

    pub fn by_id(&self) -> HashMap<i32, MethodMap> {
        self.methods.iter().map(|m| (m.id, m.clone())).collect()
    }
}
