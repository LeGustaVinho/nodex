//! Architectural views over the indexed project and the user's local declarations.

mod applications;
mod c4;
mod layers;

use std::collections::{BTreeMap, BTreeSet};

use nodex_ir::knowledge::{Confidence, EntryKind};
use nodex_ir::{Span, StructEdge, StructEdgeKind, Symbol, SymbolKind};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::index::{Index, dirname};
use crate::mermaid::label;
use crate::{Scoped, UmlError, UmlInput, UmlScope};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ArchitectureView {
    Layers,
    Context,
    Containers,
    Components,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Layer {
    Interface,
    Application,
    Domain,
    Infrastructure,
    Unclassified,
}

impl Layer {
    const ALL: [Self; 5] = [
        Self::Interface,
        Self::Application,
        Self::Domain,
        Self::Infrastructure,
        Self::Unclassified,
    ];
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
#[ts(export)]
pub struct ArchitectureConfig {
    pub layers: BTreeMap<String, Layer>,
    pub system_name: String,
    pub excluded_apps: BTreeSet<String>,
    pub excluded_stores: BTreeSet<String>,
    pub applications: Vec<DeclaredApplication>,
    pub libraries: Vec<LibraryMembership>,
    pub actors: Vec<DeclaredElement>,
    pub external_systems: Vec<DeclaredElement>,
    pub stores: Vec<DeclaredElement>,
    pub relations: Vec<DeclaredRelation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct DeclaredApplication {
    pub root: String,
    pub name: String,
    pub technology: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct LibraryMembership {
    pub application: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct DeclaredElement {
    pub id: String,
    pub name: String,
    pub technology: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct DeclaredRelation {
    pub from: String,
    pub to: String,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[ts(export)]
pub enum ArchitectureKind {
    Layer,
    Module,
    System,
    Application,
    Store,
    Component,
    External,
    Actor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[ts(export)]
pub enum Provenance {
    Detected,
    Inferred,
    Declared,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ArchitectureEvidence {
    pub text: String,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub symbol: Option<String>,
    pub span: Option<Span>,
}

impl ArchitectureEvidence {
    fn at(text: impl Into<String>, site: Option<&Span>, symbol: Option<&str>) -> Self {
        Self {
            text: text.into(),
            path: site.map(|s| s.file.clone()),
            line: site.map(|s| s.start_line),
            symbol: symbol.map(str::to_owned),
            span: site.cloned(),
        }
    }

    fn file(text: impl Into<String>, path: &str) -> Self {
        Self {
            text: text.into(),
            path: Some(path.to_owned()),
            line: None,
            symbol: None,
            span: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, TS)]
#[ts(export)]
pub struct ArchitectureNode {
    pub id: String,
    pub name: String,
    pub kind: ArchitectureKind,
    pub parent: Option<String>,
    pub provenance: Provenance,
    pub technology: Option<String>,
    pub path: Option<String>,
    pub layer: Option<Layer>,
    pub code: Option<UmlScope>,
    pub evidence: Vec<ArchitectureEvidence>,
}

impl ArchitectureNode {
    fn new(id: String, name: String, kind: ArchitectureKind, provenance: Provenance) -> Self {
        Self {
            id,
            name,
            kind,
            parent: None,
            provenance,
            technology: None,
            path: None,
            layer: None,
            code: None,
            evidence: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, TS)]
#[ts(export)]
pub struct ArchitectureEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub kind: String,
    pub label: String,
    pub provenance: Provenance,
    pub confidence: Confidence,
    pub count: u32,
    pub evidence: Vec<ArchitectureEvidence>,
}

#[derive(Clone, Debug, Serialize, TS)]
#[ts(export)]
pub struct ArchitectureDiagram {
    pub title: String,
    pub nodes: Vec<ArchitectureNode>,
    pub edges: Vec<ArchitectureEdge>,
    pub warnings: Vec<String>,
    pub pending: Vec<ArchitectureEvidence>,
    pub omitted_nodes: u32,
    pub omitted_edges: u32,
    pub mermaid: String,
    /// Applications remain available to the editor even when hidden by the boundary or view cap.
    pub applications: Vec<ArchitectureNode>,
    /// Storage candidates remain editable even when removed from the system boundary.
    pub stores: Vec<ArchitectureNode>,
    /// Logical modules are selectable when a band is collapsed or trimmed.
    pub modules: Vec<ArchitectureNode>,
    /// Older IDs and paths that resolve to a single current application or store.
    pub aliases: BTreeMap<String, String>,
}

impl ArchitectureDiagram {
    fn new(title: String) -> Self {
        Self {
            title,
            nodes: Vec::new(),
            edges: Vec::new(),
            warnings: Vec::new(),
            pending: Vec::new(),
            omitted_nodes: 0,
            omitted_edges: 0,
            mermaid: String::new(),
            applications: Vec::new(),
            stores: Vec::new(),
            modules: Vec::new(),
            aliases: BTreeMap::new(),
        }
    }

    fn finish(&mut self, relations: Relations) {
        // Producers place boundaries first, then their children, then supporting nodes.
        self.omitted_nodes = self.nodes.len().saturating_sub(60) as u32;
        self.nodes.truncate(60);
        let visible: BTreeSet<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
        let total = relations.0.len();
        self.edges = relations
            .0
            .into_values()
            .filter(|r| visible.contains(r.from.as_str()) && visible.contains(r.to.as_str()))
            .take(200)
            .collect();
        self.omitted_edges = (total - self.edges.len()) as u32;
        self.mermaid = mermaid(&self.nodes, &self.edges);
    }
}

#[derive(Default)]
struct Relations(BTreeMap<(String, String, String, Confidence, Provenance), ArchitectureEdge>);

impl Relations {
    fn add(
        &mut self,
        from: &str,
        to: &str,
        kind: &str,
        title: &str,
        confidence: Confidence,
        provenance: Provenance,
        evidence: ArchitectureEvidence,
    ) {
        if from == to {
            return;
        }
        let key = (
            from.to_owned(),
            to.to_owned(),
            if provenance == Provenance::Declared || kind == "HTTP" {
                format!("{kind}:{title}")
            } else {
                kind.to_owned()
            },
            confidence,
            provenance,
        );
        let id = format!("architecture-edge:{key:?}");
        let edge = self.0.entry(key).or_insert_with(|| ArchitectureEdge {
            id,
            from: from.into(),
            to: to.into(),
            kind: kind.into(),
            label: title.into(),
            provenance,
            confidence,
            count: 0,
            evidence: Vec::new(),
        });
        if edge.label != title {
            edge.label = kind.into();
        }
        edge.count += 1;
        push_evidence(&mut edge.evidence, evidence);
    }
}

fn push_evidence(items: &mut Vec<ArchitectureEvidence>, item: ArchitectureEvidence) {
    if items.len() < 40 && !items.contains(&item) {
        items.push(item);
    }
}

pub struct ArchitectureRequest {
    pub view: ArchitectureView,
    pub scope: UmlScope,
    pub expanded: BTreeSet<Layer>,
    pub container: Option<String>,
}

pub fn build(
    input: &UmlInput<'_>,
    request: &ArchitectureRequest,
    config: &ArchitectureConfig,
) -> Result<ArchitectureDiagram, UmlError> {
    let index = Index::new(input.graph, input.knowledge);
    let apps = applications::infer(input, &index);
    if request.view == ArchitectureView::Layers {
        layers::build(&index, input, request, config, &apps)
    } else {
        c4::build(
            &index,
            input,
            request,
            config,
            applications::apply_declarations(apps, config),
        )
    }
}

fn within(path: &str, root: &str) -> bool {
    root.is_empty() || path == root || path.strip_prefix(root).is_some_and(|s| s.starts_with('/'))
}

fn relative<'a>(path: &'a str, root: &str) -> &'a str {
    path.strip_prefix(root)
        .unwrap_or(path)
        .trim_start_matches('/')
}

fn diagram_symbol(s: &Symbol, index: &Index<'_>) -> bool {
    !s.external
        && !matches!(s.kind, SymbolKind::Namespace | SymbolKind::Package)
        && !s.origins.is_empty()
        && !index.is_test(&s.id)
}

fn module_path(file: &str, root: &str) -> String {
    let relative = relative(file, root);
    let path = relative.strip_prefix("src/").unwrap_or(relative);
    if let Some((segment, _)) = path.split_once('/') {
        let base = if relative.starts_with("src/") {
            "src/"
        } else {
            ""
        };
        let prefix = if root.is_empty() { "" } else { "/" };
        format!("{root}{prefix}{base}{segment}")
    } else {
        file.into()
    }
}

fn leaf(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn entry(index: &Index<'_>, id: &str) -> Option<EntryKind> {
    index.info.get(id)?.entry.as_ref().map(|e| e.kind)
}

fn static_relation(kind: StructEdgeKind) -> bool {
    use StructEdgeKind::*;
    matches!(
        kind,
        Inherits
            | Implements
            | Calls
            | References
            | Imports
            | Instantiates
            | DependsOn
            | Subscribes
            | Raises
    )
}

fn edge_evidence(index: &Index<'_>, edge: &StructEdge) -> ArchitectureEvidence {
    let source = index.get(&edge.from);
    let target = index.get(&edge.to);
    let text = edge.evidence.clone().unwrap_or_else(|| {
        format!(
            "{} → {} ({:?})",
            source.map_or(edge.from.as_str(), |s| &s.name),
            target.map_or(edge.to.as_str(), |s| &s.name),
            edge.kind
        )
    });
    ArchitectureEvidence::at(
        text,
        edge.site
            .as_ref()
            .or_else(|| source.and_then(|s| s.origins.first())),
        Some(&edge.from),
    )
}

fn from_confidence(confidence: Confidence) -> Provenance {
    match confidence {
        Confidence::Exact => Provenance::Detected,
        _ => Provenance::Inferred,
    }
}

fn mermaid(nodes: &[ArchitectureNode], edges: &[ArchitectureEdge]) -> String {
    fn draw(out: &mut String, nodes: &[ArchitectureNode], parent: Option<&str>, depth: usize) {
        let indent = "  ".repeat(depth);
        for (i, node) in nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.parent.as_deref() == parent)
        {
            let name = label(&format!("{} · {:?}", node.name, node.kind), 100);
            if nodes.iter().any(|n| n.parent.as_ref() == Some(&node.id)) {
                out.push_str(&format!("{indent}subgraph n{i}[\"{name}\"]\n"));
                draw(out, nodes, Some(&node.id), depth + 1);
                out.push_str(&format!("{indent}end\n"));
            } else {
                out.push_str(&format!("{indent}n{i}[\"{name}\"]\n"));
            }
        }
    }
    let mut out = String::from("flowchart TB\n");
    draw(&mut out, nodes, None, 1);
    let ids: BTreeMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    for edge in edges {
        let from = ids[edge.from.as_str()];
        let to = ids[edge.to.as_str()];
        let style = if edge.confidence == Confidence::Exact {
            "-->"
        } else {
            "-.->"
        };
        let name = label(&edge.label, 90);
        out.push_str(&format!("  n{from} {style}|\"{name}\"| n{to}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_preserve_the_boundary_and_keep_relations_between_visible_boxes() {
        let mut d = ArchitectureDiagram::new("System".into());
        d.nodes.push(ArchitectureNode::new(
            "system".into(),
            "System".into(),
            ArchitectureKind::System,
            Provenance::Declared,
        ));
        let mut relations = Relations::default();
        for i in 0..70 {
            let id = format!("app:{i:02}");
            let mut node = ArchitectureNode::new(
                id,
                i.to_string(),
                ArchitectureKind::Application,
                Provenance::Detected,
            );
            node.parent = Some("system".into());
            d.nodes.push(node);
            for to in 0..i {
                relations.add(
                    &format!("app:{i:02}"),
                    &format!("app:{to:02}"),
                    "HTTP",
                    "HTTP",
                    Confidence::Exact,
                    Provenance::Detected,
                    ArchitectureEvidence::at("request", None, None),
                );
            }
        }
        d.finish(relations);
        assert_eq!(d.nodes.len(), 60);
        assert_eq!(d.edges.len(), 200);
        assert_eq!(d.omitted_nodes, 11);
        assert_eq!(d.omitted_edges, 70 * 69 / 2 - 200);
        assert_eq!(d.nodes[0].id, "system");
        assert!(
            d.edges
                .iter()
                .all(|edge| d.nodes.iter().any(|n| n.id == edge.to)
                    && d.nodes.iter().any(|n| n.id == edge.from))
        );
    }
}
