//! Maps source byte offsets to flow nodes.

use std::collections::HashMap;

use nodex_ir::{Concept, FlowGraph};

struct FlowEntry {
    symbol: String,
    start: u32,
    end: u32,
    entry: Option<String>,
    nodes: Vec<(String, u32, u32)>,
}

/// Index of flow and node spans by file, built from the current IR.
#[derive(Default)]
pub struct Resolver {
    files: HashMap<String, Vec<FlowEntry>>,
}

/// Where a source position lands in the graphs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub flow: String,
    /// The innermost statement node containing the position (none for braces and other
    /// positions between statements).
    pub node: Option<String>,
}

impl Resolver {
    pub fn new<'a>(flows: impl IntoIterator<Item = &'a FlowGraph>) -> Self {
        let mut files: HashMap<String, Vec<FlowEntry>> = HashMap::new();
        for f in flows {
            files.entry(f.span.file.clone()).or_default().push(FlowEntry {
                symbol: f.symbol.clone(),
                start: f.span.start_byte,
                end: f.span.end_byte,
                entry: f
                    .nodes
                    .iter()
                    .find(|n| matches!(n.concept, Concept::Entry | Concept::EventEntry))
                    .map(|n| n.id.clone()),
                nodes: f.nodes.iter().map(|n| (n.id.clone(), n.span.start_byte, n.span.end_byte)).collect(),
            });
        }
        Self { files }
    }

    pub fn locate(&self, file: &str, start: u32) -> Option<Location> {
        let flow = self
            .files
            .get(file)?
            .iter()
            .filter(|f| f.start <= start && start < f.end)
            .min_by_key(|f| f.end - f.start)?;
        let node = flow
            .nodes
            .iter()
            .filter(|(_, s, e)| *s <= start && start < *e)
            .min_by_key(|(_, s, e)| e - s)
            .map(|(id, _, _)| id.clone());
        Some(Location { flow: flow.symbol.clone(), node })
    }

    /// The entry node of a flow (where arguments are shown).
    pub fn entry_node(&self, flow: &str) -> Option<String> {
        self.files.values().flatten().find(|f| f.symbol == flow).and_then(|f| f.entry.clone())
    }
}
