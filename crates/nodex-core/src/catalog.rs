//! The concept catalog (`catalog/*.toml`), embedded at compile time.

use std::collections::BTreeMap;

use nodex_ir::Concept;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

const CONCEPTS: &str = include_str!("../../../catalog/concepts.toml");
const OPERATIONS: &str = include_str!("../../../catalog/operations.toml");
const FLAVORS: &[(&str, &str)] = &[
    ("common", include_str!("../../../catalog/flavors/common.toml")),
    ("csharp", include_str!("../../../catalog/flavors/csharp.toml")),
    ("typescript", include_str!("../../../catalog/flavors/typescript.toml")),
    ("rust", include_str!("../../../catalog/flavors/rust.toml")),
];

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct CategoryInfo {
    pub title: String,
    pub color: String,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct ConceptInfo {
    pub title: String,
    pub category: String,
    pub shape: String,
    pub explain: String,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct FlavorInfo {
    pub label: String,
    pub explain: String,
}

/// A family of operations (math, text, ...): its title and node color.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct FamilyInfo {
    pub title: String,
    pub color: String,
}

/// One shared operation of `catalog/operations.toml`.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct OperationInfo {
    pub title: String,
    pub symbol: String,
    pub family: String,
    pub pure: bool,
    /// `Name:type` pins in argument order (the receiver first for methods).
    #[serde(default)]
    pub inputs: Vec<String>,
    /// `Name:type`, empty when the operation returns nothing.
    #[serde(default)]
    pub output: String,
    pub explain: String,
    #[serde(default)]
    pub cs: String,
    #[serde(default)]
    pub ts: String,
    #[serde(default)]
    pub rs: String,
    #[serde(default)]
    pub calls_cs: Vec<String>,
    #[serde(default)]
    pub calls_ts: Vec<String>,
    #[serde(default)]
    pub calls_rs: Vec<String>,
}

impl OperationInfo {
    /// The call patterns for a language (`csharp`, `typescript`, `rust`).
    pub fn calls(&self, language: &str) -> &[String] {
        match language {
            "csharp" => &self.calls_cs,
            "typescript" => &self.calls_ts,
            "rust" => &self.calls_rs,
            _ => &[],
        }
    }
}

/// Pin types an operation may use.
pub const PIN_TYPES: &[&str] = &["number", "text", "bool", "list", "map", "any", "time"];

/// Display and explanation data for concepts and flavors.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct Catalog {
    pub categories: BTreeMap<String, CategoryInfo>,
    pub concepts: BTreeMap<String, ConceptInfo>,
    /// Flavor tables keyed by source (`common`, `csharp`, ...).
    pub flavors: BTreeMap<String, BTreeMap<String, FlavorInfo>>,
    pub families: BTreeMap<String, FamilyInfo>,
    /// Shared operations keyed by `Expr::op` (`add`, `math.max`, `text.to_upper`, ...).
    pub operations: BTreeMap<String, OperationInfo>,
}

#[derive(Deserialize)]
struct OperationsFile {
    families: BTreeMap<String, FamilyInfo>,
    operations: BTreeMap<String, OperationInfo>,
}

#[derive(Deserialize)]
struct ConceptsFile {
    categories: BTreeMap<String, CategoryInfo>,
    concepts: BTreeMap<String, ConceptInfo>,
}

impl Catalog {
    /// Loads the embedded catalog. Panics only if the embedded TOML is malformed, which the
    /// test suite rules out.
    pub fn embedded() -> Self {
        Self::parse().expect("embedded catalog is valid")
    }

    pub fn parse() -> anyhow::Result<Self> {
        let file: ConceptsFile = toml::from_str(CONCEPTS)?;
        let mut flavors = BTreeMap::new();
        for (name, text) in FLAVORS {
            flavors.insert((*name).to_owned(), toml::from_str(text)?);
        }
        let ops: OperationsFile = toml::from_str(OPERATIONS)?;
        Ok(Self {
            categories: file.categories,
            concepts: file.concepts,
            flavors,
            families: ops.families,
            operations: ops.operations,
        })
    }

    /// Problems that make the catalog inconsistent with the IR. Empty when valid.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for c in Concept::ALL {
            let key = c.key();
            match self.concepts.get(&key) {
                None => problems.push(format!("concept {key} is missing from catalog/concepts.toml")),
                Some(info) if !self.categories.contains_key(&info.category) => {
                    problems.push(format!("concept {key} uses unknown category {}", info.category))
                }
                Some(_) => {}
            }
        }
        for key in self.concepts.keys() {
            if !Concept::ALL.iter().any(|c| &c.key() == key) {
                problems.push(format!("catalog concept {key} does not exist in nodex_ir::Concept"));
            }
        }
        for (key, op) in &self.operations {
            if !self.families.contains_key(&op.family) {
                problems.push(format!("operation {key} uses unknown family {}", op.family));
            }
            let output = (!op.output.is_empty()).then_some(&op.output);
            for pin in op.inputs.iter().chain(output) {
                match pin.split_once(':') {
                    Some((name, ty)) if !name.is_empty() && PIN_TYPES.contains(&ty) => {}
                    _ => problems.push(format!("operation {key} has a bad pin {pin:?} (want Name:type)")),
                }
            }
            if op.pure && op.output.is_empty() {
                problems.push(format!("pure operation {key} has no output"));
            }
            for lang in ["csharp", "typescript", "rust"] {
                for pattern in op.calls(lang) {
                    if crate::operations::Pattern::parse(pattern).is_none() {
                        problems.push(format!("operation {key} has a bad {lang} call pattern {pattern:?}"));
                    }
                }
            }
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_matches_ir() {
        let catalog = Catalog::parse().expect("catalog parses");
        assert_eq!(catalog.validate(), Vec::<String>::new());
    }
}
