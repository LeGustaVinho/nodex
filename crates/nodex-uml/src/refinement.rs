//! Typed, evidence-backed edits to a diagram presentation.

use std::collections::{BTreeMap, BTreeSet};

use nodex_ir::{Span, SymbolKind};
use serde::{Deserialize, Serialize};

use crate::architecture::{self, ArchitectureConfig, ArchitectureRequest, ArchitectureView, Layer};
use crate::{Index, Scoped, UmlDiagram, UmlError, UmlInput, UmlKind, UmlOptions, UmlScope};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    Architecture {
        view: ArchitectureView,
        scope: UmlScope,
        container: Option<String>,
    },
    Packages {
        scope: UmlScope,
    },
    Uml {
        diagram: UmlKind,
        scope: UmlScope,
        options: UmlOptions,
    },
}

impl Target {
    pub fn scope(&self) -> &UmlScope {
        match self {
            Self::Architecture { scope, .. }
            | Self::Packages { scope }
            | Self::Uml { scope, .. } => scope,
        }
    }

    pub fn normalize(&mut self) {
        let scope = match self {
            Self::Architecture {
                view,
                scope,
                container,
            } => {
                if *view != ArchitectureView::Layers {
                    *scope = UmlScope::Project;
                }
                if *view != ArchitectureView::Components {
                    *container = None;
                }
                scope
            }
            Self::Packages { scope } => scope,
            Self::Uml { options, scope, .. } => {
                options.runtime = false;
                options.confidences.sort();
                options.confidences.dedup();
                scope
            }
        };
        match scope {
            UmlScope::Files { paths } => {
                paths.sort();
                paths.dedup();
            }
            UmlScope::Symbols { ids } => {
                ids.sort();
                ids.dedup();
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub text: String,
    pub site: Option<Span>,
    pub path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub parent: Option<String>,
    pub layer: Option<Layer>,
    pub declared: bool,
    pub evidence: Vec<String>,
    pub code: Option<UmlScope>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: String,
    pub kind: String,
    pub confidence: String,
    pub count: u32,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub choice: String,
    pub edge: Edge,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inventory {
    pub title: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub evidence: Vec<Evidence>,
    pub candidates: Vec<Candidate>,
    pub pending: Vec<String>,
    pub recursive: bool,
    pub layers: bool,
    pub uml: Option<UmlDiagram>,
}

impl Inventory {
    fn record(&mut self, text: &str, site: Option<&Span>, path: Option<&str>) -> String {
        let path = path
            .or_else(|| site.map(|s| s.file.as_str()))
            .map(str::to_owned);
        let id = format!("e{}", self.evidence.len());
        self.evidence.push(Evidence {
            id: id.clone(),
            text: text.into(),
            site: site.cloned(),
            path,
        });
        id
    }

    fn architecture_evidence(
        &mut self,
        evidence: &[architecture::ArchitectureEvidence],
    ) -> Vec<String> {
        evidence
            .iter()
            .map(|e| self.record(&e.text, e.span.as_ref(), e.path.as_deref()))
            .collect()
    }
}

pub fn inventory(
    input: &UmlInput<'_>,
    target: &Target,
    config: &ArchitectureConfig,
) -> Result<Inventory, UmlError> {
    let mut result = Inventory {
        title: String::new(),
        nodes: Vec::new(),
        edges: Vec::new(),
        evidence: Vec::new(),
        candidates: Vec::new(),
        pending: Vec::new(),
        recursive: false,
        layers: false,
        uml: None,
    };
    let scope = match target {
        Target::Architecture {
            view,
            scope,
            container,
        } => {
            let d = architecture::inventory(
                input,
                &ArchitectureRequest {
                    view: *view,
                    scope: scope.clone(),
                    container: container.clone(),
                    expanded: BTreeSet::from([
                        Layer::Interface,
                        Layer::Application,
                        Layer::Domain,
                        Layer::Infrastructure,
                        Layer::Unclassified,
                    ]),
                },
                config,
            )?;
            result.title = d.title;
            result.layers = *view == ArchitectureView::Layers;
            result.recursive = result.layers || *view == ArchitectureView::Components;
            for n in d.nodes {
                let evidence = result.architecture_evidence(&n.evidence);
                let code = n.code.or_else(|| {
                    n.path.as_deref().filter(|p| !p.is_empty()).map(|path| {
                        if input
                            .graph
                            .symbols
                            .iter()
                            .any(|s| s.origins.iter().any(|o| o.file == path))
                        {
                            UmlScope::File { path: path.into() }
                        } else {
                            UmlScope::Folder { path: path.into() }
                        }
                    })
                });
                result.nodes.push(Node {
                    id: n.id,
                    label: n.name,
                    kind: format!("{:?}", n.kind),
                    parent: n.parent,
                    layer: n.layer,
                    declared: n.provenance == architecture::Provenance::Declared,
                    evidence,
                    code,
                });
            }
            let visible: BTreeSet<_> = result.nodes.iter().map(|n| n.id.clone()).collect();
            for e in d.edges {
                let evidence = result.architecture_evidence(&e.evidence);
                result.edges.push(Edge {
                    id: e.id,
                    from: e.from,
                    to: e.to,
                    label: e.label,
                    kind: e.kind,
                    count: e.count,
                    confidence: format!("{:?}", e.confidence),
                    evidence,
                });
            }
            for c in d
                .http_candidates
                .into_iter()
                .filter(|c| visible.contains(&c.from) && visible.contains(&c.to))
            {
                let Some(site) = c.evidence.first().and_then(|e| e.span.as_ref()) else {
                    continue;
                };
                let choice = format!("{}:{}:{}", c.from, site.file, site.start_byte);
                let evidence = result.architecture_evidence(&c.evidence);
                let id = format!("candidate:{}", result.candidates.len());
                result.candidates.push(Candidate {
                    id: id.clone(),
                    choice,
                    edge: Edge {
                        id,
                        from: c.from,
                        to: c.to,
                        label: c.label,
                        evidence,
                        kind: "HTTP".into(),
                        confidence: "Inferred".into(),
                        count: 1,
                    },
                });
            }
            result.pending = result.architecture_evidence(&d.pending);
            scope
        }
        Target::Packages { scope } => {
            let index = Index::new(input.graph, input.knowledge);
            let scoped = Scoped::resolve(&index, scope)?;
            let mut included = BTreeSet::new();
            for id in &scoped.symbols {
                let mut symbol = index.get(id);
                while let Some(s) = symbol {
                    if matches!(
                        s.kind,
                        SymbolKind::Namespace | SymbolKind::Module | SymbolKind::Package
                    ) {
                        included.insert(s.id.as_str());
                    }
                    symbol = s.parent.as_deref().and_then(|p| index.get(p));
                }
            }
            let imports: Vec<_> = index
                .relations()
                .filter(|e| {
                    e.kind == nodex_ir::StructEdgeKind::Imports
                        && included.contains(e.from.as_str())
                })
                .collect();
            included.extend(imports.iter().filter_map(|e| {
                index
                    .get(&e.to)
                    .filter(|s| s.external)
                    .map(|s| s.id.as_str())
            }));
            result.title = format!("Packages · {}", scoped.title);
            result.recursive = true;
            for id in &included {
                let Some(s) = index.get(id) else { continue };
                let evidence = s
                    .origins
                    .iter()
                    .map(|o| result.record(&s.qualified_name, Some(o), None))
                    .collect();
                result.nodes.push(Node {
                    id: s.id.clone(),
                    label: s.qualified_name.clone(),
                    kind: format!("{:?}", s.kind),
                    parent: None,
                    layer: None,
                    declared: false,
                    evidence,
                    code: (!s.external).then(|| UmlScope::Symbol { id: s.id.clone() }),
                });
            }
            for e in imports
                .into_iter()
                .filter(|e| included.contains(e.to.as_str()))
            {
                let evidence =
                    vec![result.record("Imports", e.site.as_ref(), index.file_of(&e.from))];
                result.edges.push(Edge {
                    id: format!("import:{}", result.edges.len()),
                    from: e.from.clone(),
                    to: e.to.clone(),
                    label: "Imports".into(),
                    kind: "Imports".into(),
                    count: 1,
                    confidence: "Exact".into(),
                    evidence,
                });
            }
            scope
        }
        Target::Uml {
            diagram,
            scope,
            options,
        } => {
            let mut options = options.clone();
            options.runtime = false;
            let d = crate::build(input, *diagram, scope, &options)?;
            result.title = d.title.clone();
            for n in &d.elements {
                let evidence = vec![result.record(
                    n.detail.as_deref().unwrap_or(&n.label),
                    n.span.as_ref(),
                    None,
                )];
                result.nodes.push(Node {
                    id: n.mermaid_id.clone(),
                    label: n.label.clone(),
                    kind: n.kind.clone(),
                    parent: None,
                    layer: None,
                    declared: false,
                    evidence,
                    code: n.symbol.clone().map(|id| UmlScope::Symbol { id }),
                });
            }
            for e in &d.relations {
                let evidence = vec![result.record(
                    e.evidence.as_deref().unwrap_or(&e.kind),
                    e.site.as_ref(),
                    None,
                )];
                result.edges.push(Edge {
                    id: format!("relation:{}", result.edges.len()),
                    from: e.from.clone(),
                    to: e.to.clone(),
                    label: e.label.clone().unwrap_or_else(|| e.kind.clone()),
                    kind: e.kind.clone(),
                    count: e.count,
                    confidence: format!(
                        "{:?}",
                        e.confidence
                            .unwrap_or(nodex_ir::knowledge::Confidence::Exact)
                    ),
                    evidence,
                });
            }
            result.uml = Some(d);
            scope
        }
    };
    if !matches!(
        target,
        Target::Architecture {
            view: ArchitectureView::Components,
            ..
        }
    ) {
        let index = Index::new(input.graph, input.knowledge);
        let scoped = Scoped::resolve(&index, scope)?;
        for s in input
            .graph
            .symbols
            .iter()
            .filter(|s| scoped.symbols.contains(&s.id) && (s.kind.is_type() || s.has_flow))
        {
            for site in s.origins.iter().filter(|o| {
                (scoped.files.is_empty() || scoped.files.contains(&o.file))
                    && scoped
                        .folder
                        .as_deref()
                        .is_none_or(|f| f.is_empty() || o.file.starts_with(&format!("{f}/")))
            }) {
                result.record(&s.qualified_name, Some(site), None);
            }
        }
        for e in index
            .relations()
            .filter(|e| scoped.symbols.contains(&e.from) && scoped.symbols.contains(&e.to))
        {
            let from = index
                .get(&e.from)
                .map_or(e.from.as_str(), |s| &s.qualified_name);
            let to = index
                .get(&e.to)
                .map_or(e.to.as_str(), |s| &s.qualified_name);
            result.record(
                &format!("{from} → {to} ({:?})", e.kind),
                e.site.as_ref(),
                index.file_of(&e.from),
            );
        }
        for entry in input
            .knowledge
            .symbols
            .iter()
            .filter(|s| scoped.symbols.contains(&s.symbol))
        {
            if let Some(entry_point) = &entry.entry
                && let Some(symbol) = index.get(&entry.symbol)
            {
                result.record(
                    &format!(
                        "Entry point: {} ({:?})",
                        symbol.qualified_name, entry_point.kind
                    ),
                    symbol.origins.first(),
                    None,
                );
            }
        }
        for unit in input
            .units
            .iter()
            .filter(|u| {
                matches!(scope, UmlScope::Project)
                    || result.evidence.iter().any(|e| {
                        e.path.as_deref().is_some_and(|p| {
                            u.root.is_empty() || p.starts_with(&format!("{}/", u.root))
                        })
                    })
            })
            .cloned()
            .collect::<Vec<_>>()
        {
            result.record(
                &format!("Build unit {}", unit.name),
                None,
                Some(&unit.manifest),
            );
        }
    }
    Ok(result)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suggestion {
    pub kind: String,
    pub target: String,
    pub text: String,
    pub members: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub suggestions: Vec<Suggestion>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reviewed {
    pub id: usize,
    pub suggestion: Suggestion,
    pub error: Option<String>,
}

pub fn review(
    base: &Inventory,
    proposal: Proposal,
    sent_evidence: &BTreeSet<String>,
    sent_nodes: &BTreeSet<String>,
    sent_edges: &BTreeSet<String>,
    sent_candidates: &BTreeSet<String>,
) -> Vec<Reviewed> {
    let groups: BTreeSet<_> = proposal
        .suggestions
        .iter()
        .filter(|s| s.kind == "group")
        .map(|s| s.target.clone())
        .collect();
    let mut reviewed: Vec<_> = proposal
        .suggestions
        .into_iter()
        .take(100)
        .enumerate()
        .map(|(id, suggestion)| {
            let s = &suggestion;
            let node = base
                .nodes
                .iter()
                .find(|n| n.id == s.target && sent_nodes.contains(&n.id));
            let edge = sent_edges.contains(&s.target);
            let typed = match s.kind.as_str() {
                "label" => node.is_some_and(|n| !n.declared) && s.text.chars().count() <= 96,
                "summary" => s.target.is_empty() || node.is_some() || edge,
                "highlight" => node.is_some() || edge,
                "layer" => {
                    base.layers
                        && node
                            .is_some_and(|n| n.layer.is_some() && n.kind == "Module" && !n.declared)
                        && parse_layer(&s.text).is_some()
                }
                "association" => sent_candidates.contains(&s.target),
                "group" => {
                    base.recursive
                        && s.target.starts_with("group:")
                        && s.target.len() <= 80
                        && !s.members.is_empty()
                        && s.members.len() <= 120
                        && s.members.iter().all(|m| {
                            groups.contains(m)
                                || base.nodes.iter().any(|n| {
                                    &n.id == m
                                        && sent_nodes.contains(m)
                                        && !matches!(
                                            n.kind.as_str(),
                                            "Layer" | "System" | "Application"
                                        )
                                })
                        })
                }
                _ => false,
            };
            let evidence_valid = s.evidence.iter().all(|e| sent_evidence.contains(e))
                && (!matches!(s.kind.as_str(), "layer" | "group" | "association")
                    || !s.evidence.is_empty());
            let grounded = match s.kind.as_str() {
                "layer" => node.is_some_and(|n| n.evidence.iter().any(|e| s.evidence.contains(e))),
                "association" => base
                    .candidates
                    .iter()
                    .find(|c| c.id == s.target)
                    .is_some_and(|c| c.edge.evidence.iter().all(|e| s.evidence.contains(e))),
                _ => true,
            };
            let valid = typed
                && evidence_valid
                && grounded
                && s.text.chars().count() <= 600
                && !s.text.trim().is_empty()
                && s.evidence.len() <= 30
                && (s.kind == "group" || s.members.is_empty());
            Reviewed {
                id,
                suggestion,
                error: (!valid).then(|| "Unsupported suggestion or insufficient evidence".into()),
            }
        })
        .collect();
    let structural: BTreeSet<_> = reviewed
        .iter()
        .filter(|r| r.error.is_none() && matches!(r.suggestion.kind.as_str(), "group" | "layer"))
        .map(|r| r.id)
        .collect();
    if reviewed
        .iter()
        .any(|r| r.suggestion.kind == "group" && r.error.is_none())
        && let Err(message) = apply(base, &reviewed, &structural)
    {
        for r in reviewed
            .iter_mut()
            .filter(|r| r.suggestion.kind == "group" && r.error.is_none())
        {
            r.error = Some(message.clone());
        }
    }
    reviewed
}

fn parse_layer(text: &str) -> Option<Layer> {
    Some(match text {
        "Interface" => Layer::Interface,
        "Application" => Layer::Application,
        "Domain" => Layer::Domain,
        "Infrastructure" => Layer::Infrastructure,
        "Unclassified" => Layer::Unclassified,
        _ => return None,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Refined {
    pub title: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub evidence: Vec<Evidence>,
    pub suggestions: Vec<Reviewed>,
    pub pending: Vec<String>,
    pub uml: Option<UmlDiagram>,
}

pub fn apply(
    base: &Inventory,
    reviewed: &[Reviewed],
    approved: &BTreeSet<usize>,
) -> Result<Refined, String> {
    let suggestions: Vec<_> = reviewed
        .iter()
        .filter(|r| approved.contains(&r.id))
        .cloned()
        .collect();
    if suggestions.len() != approved.len() || suggestions.iter().any(|r| r.error.is_some()) {
        return Err("Select only validated suggestions.".into());
    }
    let mut nodes = base.nodes.clone();
    let mut edges = base.edges.clone();
    let mut uml = base.uml.clone();
    let mut choices = BTreeSet::new();
    let mut used = BTreeSet::new();
    let mut groups = BTreeMap::new();
    let mut resolved = BTreeSet::new();
    for r in &suggestions {
        let s = &r.suggestion;
        if !used.insert((&s.kind, &s.target)) {
            return Err("Duplicate suggestions for the same target.".into());
        }
        match s.kind.as_str() {
            "label" => {
                nodes.iter_mut().find(|n| n.id == s.target).unwrap().label = s.text.clone();
                if let Some(d) = &mut uml {
                    rename_uml(d, &s.target, &s.text);
                }
            }
            "layer" => {
                let n = nodes.iter_mut().find(|n| n.id == s.target).unwrap();
                n.layer = parse_layer(&s.text);
                n.parent = Some(format!("layer:{}", s.text));
            }
            "association" => {
                let c = base.candidates.iter().find(|c| c.id == s.target).unwrap();
                if !choices.insert(&c.choice) {
                    return Err("Choose a single route for each request.".into());
                }
                edges.push(c.edge.clone());
                if let Some(site) = base
                    .evidence
                    .iter()
                    .find(|e| e.id == c.edge.evidence[0])
                    .and_then(|e| e.site.as_ref())
                {
                    resolved.insert((site.file.as_str(), site.start_byte));
                }
            }
            "group" => {
                groups.insert(s.target.as_str(), s);
            }
            _ => {}
        }
    }
    let mut membership = BTreeMap::new();
    let original_parent: BTreeMap<_, _> = nodes
        .iter()
        .map(|n| (n.id.clone(), n.parent.clone()))
        .collect();
    for (&id, group) in &groups {
        for member in &group.members {
            if (!groups.contains_key(member.as_str()) && !original_parent.contains_key(member))
                || membership.insert(member.as_str(), id).is_some()
            {
                return Err("Groups require known members with a single parent.".into());
            }
        }
    }
    let mut boundaries = BTreeMap::new();
    let mut grounded_groups = BTreeSet::new();
    for (node, boundary) in &original_parent {
        let mut cursor = node.as_str();
        let mut visited = BTreeSet::new();
        while let Some(&parent) = membership.get(cursor) {
            if !visited.insert(parent) {
                return Err("Groups form a cycle.".into());
            }
            if boundaries
                .insert(parent, boundary)
                .is_some_and(|existing| existing != boundary)
            {
                return Err("A group must stay within a single architecture boundary.".into());
            }
            let evidence = &groups[parent].evidence;
            if base
                .nodes
                .iter()
                .find(|n| &n.id == node)
                .is_some_and(|n| n.evidence.iter().any(|e| evidence.contains(e)))
            {
                grounded_groups.insert(parent);
            }
            cursor = parent;
        }
    }
    if boundaries.len() != groups.len() {
        return Err("Each group needs a path to at least one diagram element.".into());
    }
    if grounded_groups.len() != groups.len() {
        return Err("Each group needs evidence from one of its members.".into());
    }
    if let Some(d) = &mut uml {
        let mut highlighted = BTreeSet::new();
        for suggestion in suggestions
            .iter()
            .map(|r| &r.suggestion)
            .filter(|s| s.kind == "highlight")
        {
            if let Some(e) = base.edges.iter().find(|e| e.id == suggestion.target) {
                highlighted.extend([e.from.as_str(), e.to.as_str()]);
            } else {
                highlighted.insert(&suggestion.target);
            }
        }
        for id in highlighted {
            if let Some(label) = d
                .elements
                .iter()
                .find(|e| e.mermaid_id == id)
                .map(|n| format!("★ {}", n.label))
            {
                rename_uml(d, id, &label);
            }
        }
    }
    if base.layers
        && nodes
            .iter()
            .any(|n| n.parent.as_deref() == Some("layer:Unclassified"))
        && !nodes.iter().any(|n| n.id == "layer:Unclassified")
    {
        nodes.push(Node {
            id: "layer:Unclassified".into(),
            label: "Unclassified".into(),
            kind: "Layer".into(),
            parent: None,
            layer: Some(Layer::Unclassified),
            declared: false,
            evidence: Vec::new(),
            code: None,
        });
    }
    for node in &mut nodes {
        if let Some(parent) = membership.get(node.id.as_str()) {
            node.parent = Some((*parent).into());
        }
    }
    for (id, group) in groups {
        nodes.push(Node {
            id: id.into(),
            label: group.text.clone(),
            kind: "Group".into(),
            parent: membership
                .get(id)
                .map(|p| (*p).into())
                .or_else(|| boundaries[id].clone()),
            layer: None,
            declared: false,
            evidence: group.evidence.clone(),
            code: None,
        });
    }
    let pending = base
        .pending
        .iter()
        .filter(|id| {
            base.evidence
                .iter()
                .find(|e| &e.id == *id)
                .and_then(|e| e.site.as_ref())
                .is_none_or(|s| !resolved.contains(&(s.file.as_str(), s.start_byte)))
        })
        .cloned()
        .collect();
    Ok(Refined {
        title: base.title.clone(),
        nodes,
        edges,
        evidence: base.evidence.clone(),
        suggestions,
        pending,
        uml,
    })
}

// Replace only a local renderer's declaration label. IDs, UML operators and the
// body of classes or behavioral diagrams remain in the original syntax.
fn rename_uml(diagram: &mut UmlDiagram, id: &str, name: &str) {
    let mut safe = String::new();
    for ch in crate::mermaid::shorten(name, 96).chars() {
        match ch {
            ';' => safe.push_str("#59;"),
            '|' => safe.push_str("#124;"),
            '`' => safe.push_str("#96;"),
            '\\' => safe.push_str("#92;"),
            '#' => safe.push_str("#35;"),
            '"' => safe.push_str("#quot;"),
            '<' => safe.push_str("#lt;"),
            '>' => safe.push_str("#gt;"),
            c if c.is_control() => safe.push(' '),
            c => safe.push(c),
        }
    }
    let mut changed = false;
    diagram.mermaid = diagram
        .mermaid
        .split_inclusive('\n')
        .map(|line| {
            let trimmed = line.trim_start();
            let prefix = line.len() - trimmed.len();
            let declaration = ["", "class ", "subgraph "].into_iter().any(|start| {
                trimmed
                    .strip_prefix(start)
                    .and_then(|part| part.strip_prefix(id))
                    .is_some_and(|part| part.starts_with(['[', '(']) || part.starts_with(" ["))
            });
            let state = trimmed.starts_with("state \"")
                && trimmed.trim_end().ends_with(&format!("\" as {id}"));
            if declaration || state {
                if let Some(first) = line[prefix..].find('"').map(|i| prefix + i + 1)
                    && let Some(last) = line[first..].find('"').map(|i| first + i)
                {
                    changed = true;
                    return format!("{}{safe}{}", &line[..first], &line[last..]);
                }
            }
            for keyword in ["participant ", "actor ", "create participant "] {
                if trimmed.starts_with(&format!("{keyword}{id} as ")) {
                    changed = true;
                    return format!("{}{keyword}{id} as {safe}\n", &line[..prefix]);
                }
            }
            line.to_owned()
        })
        .collect();
    if changed && let Some(n) = diagram.elements.iter_mut().find(|n| n.mermaid_id == id) {
        n.label = name.into();
    }
}
