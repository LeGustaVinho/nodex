use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use nodex_uml::refinement::{Evidence, Inventory};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(super) const BUDGET: usize = 96 * 1024;
const SNIPPET_LINES: usize = 36;
pub(crate) const DOCUMENTS: [&str; 2] = ["README.md", "ARCHITECTURE.md"];

#[derive(Clone)]
pub(super) struct Context {
    pub input: String,
    pub snippets: Vec<Snippet>,
    pub nodes: BTreeSet<String>,
    pub edges: BTreeSet<String>,
    pub evidence: BTreeSet<String>,
    pub omitted: usize,
    aliases: BTreeMap<String, String>,
}

impl Context {
    pub fn resolve(
        &self,
        mut proposal: nodex_uml::refinement::Proposal,
    ) -> nodex_uml::refinement::Proposal {
        for suggestion in &mut proposal.suggestions {
            if let Some(id) = self.aliases.get(&suggestion.target) {
                suggestion.target.clone_from(id);
            }
            for member in &mut suggestion.members {
                if let Some(id) = self.aliases.get(member) {
                    member.clone_from(id);
                }
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
    pub revision: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum ExcludedExcerpt {
    Region(ExcerptRegion),
    EvidenceId(String),
}

#[derive(Deserialize)]
pub(super) struct ExcerptRegion {
    pub path: String,
    pub line: usize,
    #[serde(default)]
    pub revision: Option<String>,
}

pub(super) fn path_allowed(path: &str) -> bool {
    !path.contains(['\\', ':'])
        && Path::new(path).components().all(|part| {
            let Component::Normal(part) = part else {
                return false;
            };
            let name = part.to_string_lossy().to_ascii_lowercase();
            !name.starts_with(".env")
                && ![
                    ".git",
                    ".nodex",
                    ".ssh",
                    ".aws",
                    ".azure",
                    ".npmrc",
                    ".netrc",
                    ".pypirc",
                    "id_rsa",
                    "id_ed25519",
                ]
                .contains(&name.as_str())
                && !["credential", "secret", "keystore"]
                    .iter()
                    .any(|x| name.contains(x))
                && ![".pem", ".key", ".pfx", ".p12", ".kdbx"]
                    .iter()
                    .any(|x| name.ends_with(x))
        })
}

pub(super) fn scrub(text: &str) -> String {
    text.lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            let secret = [
                "password",
                "passwd",
                "secret",
                "apikey",
                "api_key",
                "access_token",
                "authtoken",
                "private_key",
                "authorization",
                "connectionstring",
                "connection_string",
            ]
            .iter()
            .any(|word| lower.contains(word))
                && line.contains(['=', ':', '"', '\'']);
            let credential = [
                "-----begin ",
                "bearer ",
                "ghp_",
                "github_pat_",
                "sk-proj-",
                "sk-ant-",
                "akia",
                "eyj",
            ]
            .iter()
            .any(|token| lower.contains(token));
            if secret || credential || (line.contains("://") && line.contains(['@', '?'])) {
                "[redacted]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// Resolve canonical paths from this project rather than trusting paths
// returned by the provider or supplied by the diagram client.
fn source_path(root: &Path, relative: &str) -> Option<PathBuf> {
    if !path_allowed(relative) {
        return None;
    }
    let file = root.join(relative).canonicalize().ok()?;
    let canonical_root = root.canonicalize().ok()?;
    let path = file.strip_prefix(&canonical_root).ok()?;
    if !path_allowed(&path.to_string_lossy().replace('\\', "/"))
        || !file.is_file()
        || std::fs::metadata(&file).ok()?.len() > 2 * 1024 * 1024
    {
        return None;
    }
    Some(file)
}

pub(super) fn source(root: &Path, relative: &str) -> Option<String> {
    std::fs::read_to_string(source_path(root, relative)?).ok()
}

fn snippet_start(evidence: &Evidence) -> usize {
    evidence
        .site
        .as_ref()
        .map_or(1, |s| (s.start_line as usize).saturating_sub(3).max(1))
}

pub(super) fn evidence_path(evidence: &Evidence) -> Option<&str> {
    evidence
        .site
        .as_ref()
        .map(|span| span.file.as_str())
        .or(evidence.path.as_deref())
}

#[derive(Default)]
struct ExcludedFile {
    source: Option<String>,
    revision: Option<String>,
    hide_all: bool,
    starts: BTreeSet<usize>,
}

impl ExcludedFile {
    fn overlaps(&self, first: usize, last: usize) -> bool {
        self.hide_all
            || self
                .starts
                .iter()
                .any(|start| first < start.saturating_add(SNIPPET_LINES) && *start <= last)
    }
}

fn excluded_files(
    root: &Path,
    excluded: &[ExcerptRegion],
) -> (BTreeMap<PathBuf, ExcludedFile>, BTreeSet<String>) {
    let mut files = BTreeMap::new();
    let mut missing = BTreeSet::new();
    for ExcerptRegion {
        path,
        line,
        revision,
    } in excluded
    {
        let Some(path_on_disk) = source_path(root, path) else {
            missing.insert(path.to_owned());
            continue;
        };
        let file = files.entry(path_on_disk.clone()).or_insert_with(|| {
            let source = std::fs::read_to_string(&path_on_disk).ok();
            let revision = source.as_ref().map(super::hash);
            ExcludedFile {
                source,
                revision,
                ..Default::default()
            }
        });
        if revision.is_some() && revision == &file.revision {
            file.starts.insert(*line);
        } else {
            file.hide_all = true;
        }
    }
    (files, missing)
}

pub(super) fn pack(
    base: &Inventory,
    root: &Path,
    excluded: &[ExcerptRegion],
    focus: Option<&str>,
) -> Context {
    let (excluded_files, unavailable_paths) = excluded_files(root, excluded);
    let mut paths = BTreeMap::new();
    let mut withheld = BTreeSet::new();
    if !excluded.is_empty() {
        for e in &base.evidence {
            let Some(path) = evidence_path(e) else {
                continue;
            };
            let file = paths.entry(path).or_insert_with(|| source_path(root, path));
            if unavailable_paths.contains(path)
                || file
                    .as_ref()
                    .and_then(|path| excluded_files.get(path))
                    .is_some_and(|file| match &e.site {
                        Some(span) => {
                            file.overlaps(span.start_line as usize, span.end_line as usize)
                        }
                        None => file.hide_all || !file.starts.is_empty(),
                    })
            {
                withheld.insert(e.id.as_str());
            }
        }
    }
    let includes_withheld = |ids: &[String]| ids.iter().any(|id| withheld.contains(id.as_str()));
    let visible_evidence = |ids: &[String]| -> Vec<String> {
        ids.iter()
            .filter(|id| !withheld.contains(id.as_str()))
            .cloned()
            .collect()
    };
    let context_label = |label: &str, ids: &[String]| {
        if includes_withheld(ids) {
            "[excluded]".into()
        } else {
            scrub(label)
        }
    };
    let mut remaining = BUDGET - 1024;
    let mut omitted = 0;
    let mut insert = |items: &mut Vec<Value>, item: Value, reserve: usize| {
        let cost = item.to_string().len() + 1;
        if cost > remaining.saturating_sub(reserve) {
            omitted += 1;
            return false;
        }
        remaining -= cost;
        items.push(item);
        true
    };
    let mut nodes = BTreeSet::new();
    let mut edges = BTreeSet::new();
    let mut candidate_evidence = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut snippets = Vec::new();
    let mut node_items = Vec::new();
    let mut edge_items = Vec::new();
    let mut candidate_items = Vec::new();
    let mut evidence_items = Vec::new();
    let mut snippet_items = Vec::new();
    let mut selected = vec![Vec::new(); 3];
    let node_aliases: BTreeMap<_, _> = base
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), format!("node:{i}")))
        .collect();
    let mut aliases = BTreeMap::new();
    let adjacent: BTreeSet<&str> = base
        .edges
        .iter()
        .filter_map(|e| {
            if Some(e.from.as_str()) == focus {
                Some(e.to.as_str())
            } else if Some(e.to.as_str()) == focus {
                Some(e.from.as_str())
            } else {
                None
            }
        })
        .collect();
    for n in &base.nodes {
        let rank = if Some(n.id.as_str()) == focus {
            0
        } else if focus.is_none()
            || Some(n.parent.as_deref().unwrap_or("")) == focus
            || adjacent.contains(n.id.as_str())
        {
            1
        } else {
            2
        };
        selected[rank].push(n);
    }
    for n in selected.into_iter().flatten() {
        let alias = &node_aliases[n.id.as_str()];
        if insert(
            &mut node_items,
            json!({"id":alias,"label":context_label(&n.label,&n.evidence),"kind":n.kind,
            "parent":n.parent.as_deref().and_then(|id| node_aliases.get(id)),
            "layer":n.layer,"declared":n.declared,"evidence":visible_evidence(&n.evidence)}),
            55 * 1024,
        ) {
            aliases.insert(alias.clone(), n.id.clone());
            nodes.insert(n.id.clone());
        }
    }
    for c in &base.candidates {
        if nodes.contains(&c.edge.from)
            && nodes.contains(&c.edge.to)
            && insert(
                &mut candidate_items,
                json!({"id":c.id,"from":node_aliases[c.edge.from.as_str()],"to":node_aliases[c.edge.to.as_str()],
                "label":context_label(&c.edge.label,&c.edge.evidence),"confidence":c.edge.confidence,"evidence":visible_evidence(&c.edge.evidence)}),
                44 * 1024,
            )
        {
            candidate_evidence.extend(c.edge.evidence.iter().map(String::as_str));
        }
    }
    for (i, e) in base.edges.iter().enumerate() {
        let alias = format!("edge:{i}");
        if nodes.contains(&e.from)
            && nodes.contains(&e.to)
            && insert(
                &mut edge_items,
                json!({"id":alias,"from":node_aliases[e.from.as_str()],"to":node_aliases[e.to.as_str()],
                "label":context_label(&e.label,&e.evidence),"kind":e.kind,"confidence":e.confidence,"count":e.count,"evidence":visible_evidence(&e.evidence)}),
                36 * 1024,
            )
        {
            aliases.insert(alias, e.id.clone());
            edges.insert(e.id.clone());
        }
    }
    let important: BTreeSet<&str> = base
        .nodes
        .iter()
        .filter(|n| Some(n.id.as_str()) == focus)
        .flat_map(|n| n.evidence.iter().map(String::as_str))
        .chain(candidate_evidence)
        .collect();
    let mut sorted: Vec<_> = base.evidence.iter().collect();
    let node_evidence: BTreeSet<&str> = base
        .nodes
        .iter()
        .filter(|n| nodes.contains(&n.id))
        .flat_map(|n| n.evidence.iter().map(String::as_str))
        .collect();
    sorted.sort_by_key(|e| {
        if important.contains(e.id.as_str()) {
            0
        } else if e.text == "Project documentation" {
            1
        } else if node_evidence.contains(e.id.as_str()) {
            2
        } else {
            3
        }
    });
    for e in &sorted {
        if withheld.contains(e.id.as_str()) || evidence_path(e).is_some_and(|p| !path_allowed(p)) {
            continue;
        }
        if insert(
            &mut evidence_items,
            json!({"id":e.id,"text":scrub(&e.text),"file":evidence_path(e),"line":e.site.as_ref().map(|s|s.start_line)}),
            14 * 1024,
        ) {
            evidence.insert(e.id.clone());
        }
    }
    let mut snippet_files = BTreeSet::new();
    for e in sorted {
        if snippets.len() >= 16 {
            break;
        }
        if !evidence.contains(&e.id) {
            continue;
        }
        let Some(path) = evidence_path(e) else {
            continue;
        };
        let Some(file) = paths
            .entry(path)
            .or_insert_with(|| source_path(root, path))
            .as_ref()
        else {
            continue;
        };
        let line = snippet_start(e);
        if snippet_files.contains(file)
            || excluded_files.get(file).is_some_and(|excluded| {
                excluded.overlaps(line, line.saturating_add(SNIPPET_LINES - 1))
            })
        {
            continue;
        }
        let buffer;
        let source = match excluded_files.get(file) {
            Some(excluded) => excluded.source.as_deref(),
            None => {
                buffer = std::fs::read_to_string(file).ok();
                buffer.as_deref()
            }
        };
        let Some(source) = source else { continue };
        let text = scrub(
            &source
                .lines()
                .skip(line - 1)
                .take(SNIPPET_LINES)
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let text: String = text.chars().take(2400).collect();
        let snippet = Snippet {
            id: e.id.clone(),
            path: path.to_owned(),
            line,
            text,
            revision: super::hash(&source),
        };
        if !snippet.text.is_empty()
            && insert(
                &mut snippet_items,
                json!({
                    "id":snippet.id, "path":snippet.path, "line":snippet.line, "text":snippet.text,
                }),
                0,
            )
        {
            snippet_files.insert(file.clone());
            snippets.push(snippet);
        }
    }
    let title = if withheld.is_empty() {
        scrub(&base.title)
    } else {
        "Diagram".into()
    };
    let input = json!({"title":title,"focus":focus.and_then(|id| node_aliases.get(id)),"recursive":base.recursive,"layers":base.layers,
        "nodes":node_items,"relationships":edge_items,"candidates":candidate_items,"evidence":evidence_items,
        "pending":base.pending.iter().filter(|id| evidence.contains(*id)).collect::<Vec<_>>(),"excerpts":snippet_items}).to_string();
    Context {
        input,
        snippets,
        nodes,
        edges,
        evidence,
        omitted,
        aliases,
    }
}
