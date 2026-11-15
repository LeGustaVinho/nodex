use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use nodex_uml::refinement::Inventory;
use serde::Serialize;
use serde_json::{Value, json};

pub(super) const BUDGET: usize = 96 * 1024;

#[derive(Clone)]
pub(super) struct Context {
    pub input: String,
    pub snippets: Vec<Snippet>,
    pub nodes: BTreeSet<String>,
    pub edges: BTreeSet<String>,
    pub candidates: BTreeSet<String>,
    pub evidence: BTreeSet<String>,
    pub omitted: usize,
    aliases: BTreeMap<String, String>,
}

impl Context {
    pub fn resolve(&self, mut proposal: nodex_uml::refinement::Proposal) -> nodex_uml::refinement::Proposal {
        for suggestion in &mut proposal.suggestions {
            if let Some(id) = self.aliases.get(&suggestion.target) { suggestion.target.clone_from(id); }
            for member in &mut suggestion.members {
                if let Some(id) = self.aliases.get(member) { member.clone_from(id); }
            }
        }
        proposal
    }
}

#[derive(Clone, Serialize)]
pub(super) struct Snippet {
    pub id: String,
    pub path: String,
    pub line: usize,
    pub text: String,
}

pub(super) fn path_allowed(path: &str) -> bool {
    !path.contains(['\\', ':']) && Path::new(path).components().all(|part| {
        let Component::Normal(part) = part else { return false };
        let name = part.to_string_lossy().to_ascii_lowercase();
        !name.starts_with(".env")
            && ![".git", ".nodex", ".ssh", ".aws", ".azure", ".npmrc", ".netrc", ".pypirc", "id_rsa", "id_ed25519"].contains(&name.as_str())
            && !["credential", "secret", "keystore"].iter().any(|x| name.contains(x))
            && ![".pem", ".key", ".pfx", ".p12", ".kdbx"].iter().any(|x| name.ends_with(x))
    })
}

pub(super) fn scrub(text: &str) -> String {
    text.lines().map(|line| {
        let lower = line.to_ascii_lowercase();
        let secret = ["password", "passwd", "secret", "apikey", "api_key", "access_token", "authtoken", "private_key", "authorization", "connectionstring", "connection_string"]
            .iter().any(|word| lower.contains(word)) && line.contains(['=', ':', '"', '\'']);
        let credential = ["-----begin ", "bearer ", "ghp_", "github_pat_", "sk-proj-", "sk-ant-", "akia", "eyj"]
            .iter().any(|token| lower.contains(token));
        if secret || credential || (line.contains("://") && line.contains(['@', '?'])) {
            "[redacted]"
        } else {
            line
        }
    }).collect::<Vec<_>>().join("\n")
}

// Resolve canonical paths from this project rather than trusting paths
// returned by the provider or supplied by the diagram client.
pub(super) fn source(root: &Path, relative: &str) -> Option<String> {
    if !path_allowed(relative) { return None; }
    let file = root.join(relative).canonicalize().ok()?;
    let canonical_root = root.canonicalize().ok()?;
    let path = file.strip_prefix(&canonical_root).ok()?;
    if !path_allowed(&path.to_string_lossy().replace('\\', "/")) || !file.is_file()
        || std::fs::metadata(&file).ok()?.len() > 2 * 1024 * 1024
    {
        return None;
    }
    std::fs::read_to_string(file).ok()
}

pub(super) fn pack(base: &Inventory, root: &Path, excluded: &BTreeSet<String>, focus: Option<&str>) -> Context {
    let mut remaining = BUDGET - 1024;
    let mut omitted = 0;
    let mut insert = |items: &mut Vec<Value>, item: Value| {
        let cost = item.to_string().len() + 1;
        if cost > remaining { omitted += 1; return false; }
        remaining -= cost;
        items.push(item);
        true
    };
    let mut nodes = BTreeSet::new();
    let mut edges = BTreeSet::new();
    let mut candidates = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut snippets = Vec::new();
    let mut node_items = Vec::new();
    let mut edge_items = Vec::new();
    let mut candidate_items = Vec::new();
    let mut evidence_items = Vec::new();
    let mut snippet_items = Vec::new();
    let mut selected = vec![Vec::new(); 3];
    let node_aliases: BTreeMap<_, _> = base.nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), format!("node:{i}"))).collect();
    let mut aliases = BTreeMap::new();
    for n in &base.nodes {
        let rank = if Some(n.id.as_str()) == focus { 0 }
        else if focus.is_none() || Some(n.parent.as_deref().unwrap_or("")) == focus
            || base.edges.iter().any(|e| (Some(e.from.as_str()) == focus && e.to == n.id) || (Some(e.to.as_str()) == focus && e.from == n.id)) { 1 }
        else { 2 };
        selected[rank].push(n);
    }
    for n in selected.into_iter().flatten() {
        let alias = &node_aliases[n.id.as_str()];
        if insert(&mut node_items, json!({"id":alias,"label":scrub(&n.label),"kind":n.kind,
            "parent":n.parent.as_deref().and_then(|id| node_aliases.get(id)),
            "layer":n.layer,"declared":n.declared,"evidence":n.evidence})) {
            aliases.insert(alias.clone(), n.id.clone());
            nodes.insert(n.id.clone());
        }
    }
    for c in &base.candidates {
        if nodes.contains(&c.edge.from) && nodes.contains(&c.edge.to)
            && insert(&mut candidate_items, json!({"id":c.id,"from":node_aliases[c.edge.from.as_str()],"to":node_aliases[c.edge.to.as_str()],
                "label":scrub(&c.edge.label),"evidence":c.edge.evidence})) { candidates.insert(c.id.clone()); }
    }
    for (i, e) in base.edges.iter().enumerate() {
        let alias = format!("edge:{i}");
        if nodes.contains(&e.from) && nodes.contains(&e.to)
            && insert(&mut edge_items, json!({"id":alias,"from":node_aliases[e.from.as_str()],"to":node_aliases[e.to.as_str()],
                "label":scrub(&e.label),"kind":e.kind,"confidence":e.confidence,"count":e.count,"evidence":e.evidence})) {
            aliases.insert(alias, e.id.clone());
            edges.insert(e.id.clone());
        }
    }
    let important: BTreeSet<&str> = base.nodes.iter().filter(|n| Some(n.id.as_str()) == focus).flat_map(|n| n.evidence.iter().map(String::as_str))
        .chain(base.candidates.iter().filter(|c| candidates.contains(&c.id)).flat_map(|c| c.edge.evidence.iter().map(String::as_str))).collect();
    let mut sorted: Vec<_> = base.evidence.iter().collect();
    sorted.sort_by_key(|e| !important.contains(e.id.as_str()));
    for e in &sorted {
        if e.path.as_deref().is_some_and(|p| !path_allowed(p)) { continue; }
        if insert(&mut evidence_items, json!({"id":e.id,"text":scrub(&e.text),"file":e.path,"line":e.site.as_ref().map(|s|s.start_line)})) {
            evidence.insert(e.id.clone());
        }
    }
    let mut source_cache = BTreeMap::new();
    let mut opened = BTreeSet::new();
    for e in sorted {
        if !evidence.contains(&e.id) || excluded.contains(&e.id) || snippets.len() >= 16 { continue; }
        let Some(path) = &e.path else { continue };
        if !opened.insert(path) { continue; }
        let source = source_cache.entry(path).or_insert_with(|| source(root, path));
        let Some(source) = source else { continue };
        let line = e.site.as_ref().map_or(1, |s| (s.start_line as usize).saturating_sub(3).max(1));
        let text = scrub(&source.lines().skip(line - 1).take(36).collect::<Vec<_>>().join("\n"));
        let text: String = text.chars().take(2400).collect();
        let snippet = Snippet { id: e.id.clone(), path: path.clone(), line, text };
        if !snippet.text.is_empty() && insert(&mut snippet_items, json!(snippet)) { snippets.push(snippet); }
    }
    let input = json!({"title":scrub(&base.title),"focus":focus.and_then(|id| node_aliases.get(id)),"recursive":base.recursive,"layers":base.layers,
        "nodes":node_items,"relationships":edge_items,"candidates":candidate_items,"evidence":evidence_items,
        "pending":base.pending.iter().filter(|id| evidence.contains(*id)).collect::<Vec<_>>(),"excerpts":snippet_items}).to_string();
    Context { input, snippets, nodes, edges, candidates, evidence, omitted, aliases }
}
