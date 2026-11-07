//! The concept catalog (`catalog/*.toml`), embedded at compile time.

use std::collections::BTreeMap;

use nodex_ir::Concept;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

const CONCEPTS: &str = include_str!("../../../catalog/concepts.toml");
const FLAVORS: &[(&str, &str)] = &[
    ("common", include_str!("../../../catalog/flavors/common.toml")),
    ("csharp", include_str!("../../../catalog/flavors/csharp.toml")),
    ("typescript", include_str!("../../../catalog/flavors/typescript.toml")),
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

/// Display and explanation data for concepts and flavors.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct Catalog {
    pub categories: BTreeMap<String, CategoryInfo>,
    pub concepts: BTreeMap<String, ConceptInfo>,
    /// Flavor tables keyed by source (`common`, `csharp`, ...).
    pub flavors: BTreeMap<String, BTreeMap<String, FlavorInfo>>,
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
        Ok(Self { categories: file.categories, concepts: file.concepts, flavors })
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
