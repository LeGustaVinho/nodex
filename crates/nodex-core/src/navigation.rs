//! Project-local source navigation, indexed alongside the structure graph.

use std::collections::{HashMap, HashSet};
use std::io::{self, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nodex_ir::knowledge::Confidence;
use nodex_ir::semantic::{ByteRange, RefKind, SemanticFacts, SemanticState};
use nodex_ir::{Language, Span, StructEdgeKind, StructureGraph, Symbol, SymbolKind};
use nodex_lang_api::kit::LineIndex;
use serde::{Deserialize, Serialize};

use crate::cache::Stat;
use crate::semantic::Locator;

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NavigationKind {
    Usages,
    Declaration,
    Implementations,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Coverage {
    Indexed,
    Loading,
    Partial,
}

#[derive(Clone)]
pub(crate) struct Hit {
    pub site: ByteRange,
    symbol: Option<String>,
    approximate: bool,
}

impl Hit {
    fn declaration(span: &Span, symbol: &str, approximate: bool) -> Self {
        Self {
            site: ByteRange {
                file: span.file.clone(),
                start: span.start_byte,
                end: span.end_byte,
            },
            symbol: Some(symbol.to_owned()),
            approximate,
        }
    }
}

pub(crate) struct NavigationIndex {
    graph: Arc<StructureGraph>,
    symbols: HashMap<String, usize>,
    usages: HashMap<String, Vec<Hit>>,
    implementations: HashMap<String, Vec<(String, Hit)>>,
    indexed: HashSet<Language>,
}

impl NavigationIndex {
    pub fn new(graph: Arc<StructureGraph>, locator: &Locator, semantic: &[(Language, Arc<SemanticFacts>)]) -> Self {
        let symbols = graph
            .symbols
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.clone(), i))
            .collect();
        let indexed = semantic
            .iter()
            .filter(|(_, facts)| facts.navigation.is_some())
            .map(|(lang, _)| *lang)
            .collect();
        let mut index = Self {
            graph,
            symbols,
            usages: HashMap::new(),
            implementations: HashMap::new(),
            indexed,
        };
        let mut exact_pairs = HashSet::new();
        for (language, facts) in semantic {
            let is_indexed = index.indexed.contains(language);
            for fact in facts.navigation.iter().flatten() {
                let Some(target) = locator.navigation_target(&fact.target) else {
                    continue;
                };
                let symbol = locator.owner_id(&fact.site).map(str::to_owned);
                let hit = Hit {
                    site: fact.site.clone(),
                    symbol,
                    approximate: false,
                };
                if fact.implementation {
                    let kind = fact.implementation_kind.as_deref().unwrap_or(&fact.target.kind);
                    let Some(from) = locator.navigation_at(&fact.site, kind, &fact.target.display) else {
                        continue;
                    };
                    if from != target {
                        let hit = Hit {
                            symbol: Some(from.clone()),
                            ..hit
                        };
                        index.implementations.entry(target).or_default().push((from, hit));
                    }
                } else {
                    index.usages.entry(target).or_default().push(hit);
                }
            }
            for fact in &facts.refs {
                let Some(target) = locator.target(&fact.target) else {
                    continue;
                };
                // Navigation providers already supply checked heritage links.
                if (fact.kind == RefKind::Compatible || (fact.kind == RefKind::Base && !is_indexed))
                    && let Some(from) = locator.type_at(&fact.site)
                    && from != target
                    && let Some(span) = index.symbol(from).and_then(|s| {
                        s.origins.iter().find(|span| {
                            span.file == fact.site.file
                                && span.start_byte <= fact.site.start
                                && span.end_byte >= fact.site.end
                        })
                    })
                {
                    let hit = Hit::declaration(span, from, false);
                    index
                        .implementations
                        .entry(target.clone())
                        .or_default()
                        .push((from.to_owned(), hit));
                }
                if !is_indexed && fact.kind != RefKind::Compatible {
                    let symbol = locator.owner_id(&fact.site).map(str::to_owned);
                    if let Some(owner) = &symbol {
                        exact_pairs.insert((owner.clone(), target.clone()));
                    }
                    index.usages.entry(target).or_default().push(Hit {
                        site: fact.site.clone(),
                        symbol,
                        approximate: false,
                    });
                }
            }
        }
        // For providers without navigation facts, keep the linked relations and make clear when
        // the destination is the owner's declaration instead of the exact reference.
        let graph = index.graph.clone();
        let mut children: HashMap<&str, Vec<&Symbol>> = HashMap::new();
        let mut rust_trait_names = HashMap::new();
        for symbol in &graph.symbols {
            if let Some(parent) = symbol.parent.as_deref() {
                children.entry(parent).or_default().push(symbol);
            }
            if symbol.language == Language::Rust && symbol.kind == SymbolKind::Trait {
                *rust_trait_names.entry(symbol.name.as_str()).or_insert(0) += 1;
            }
        }
        let mut trait_names = HashMap::new();
        for edge in graph
            .edges
            .iter()
            .filter(|edge| edge.kind == StructEdgeKind::Implements)
        {
            if let Some(target) = index.symbol(&edge.to).filter(|s| s.kind == SymbolKind::Trait) {
                *trait_names
                    .entry((edge.from.as_str(), target.name.clone()))
                    .or_insert(0) += 1;
            }
        }
        for edge in &graph.edges {
            let (Some(&from), Some(&to)) = (index.symbols.get(&edge.from), index.symbols.get(&edge.to)) else {
                continue;
            };
            let (from, to) = (&graph.symbols[from], &graph.symbols[to]);
            if index.indexed.contains(&to.language) || from.external || to.external {
                continue;
            }
            let Some(origin) = from.origins.first() else { continue };
            if matches!(
                edge.kind,
                StructEdgeKind::Inherits | StructEdgeKind::Implements | StructEdgeKind::Compatible
            ) {
                let hit = Hit::declaration(origin, &from.id, true);
                index
                    .implementations
                    .entry(to.id.clone())
                    .or_default()
                    .push((from.id.clone(), hit));
                if to.kind == SymbolKind::Trait
                    && edge.kind == StructEdgeKind::Implements
                    && trait_names.get(&(edge.from.as_str(), to.name.clone())) == Some(&1)
                {
                    let marker = format!("from-trait:{}", to.name);
                    for member in children
                        .get(from.id.as_str())
                        .into_iter()
                        .flatten()
                        .filter(|s| s.flavors.contains(&marker))
                    {
                        if member.kind == SymbolKind::TypeAlias && to.language == Language::Rust
                            && !associated_trait_matches(member, to, rust_trait_names.get(to.name.as_str()) == Some(&1))
                        {
                            continue;
                        }
                        let Some(site) = member.origins.first() else { continue };
                        let matches = children
                            .get(to.id.as_str())
                            .into_iter()
                            .flatten()
                            .filter(|s| s.name == member.name && s.kind == member.kind);
                        for contract in matches {
                            let hit = Hit::declaration(site, &member.id, true);
                            index
                                .implementations
                                .entry(contract.id.clone())
                                .or_default()
                                .push((member.id.clone(), hit));
                        }
                    }
                }
            }
            if matches!(
                edge.kind,
                StructEdgeKind::Calls
                    | StructEdgeKind::References
                    | StructEdgeKind::Instantiates
                    | StructEdgeKind::Inherits
                    | StructEdgeKind::Implements
                    | StructEdgeKind::Raises
                    | StructEdgeKind::Mutates
            ) && edge.confidence != Some(Confidence::Unresolved)
                && !exact_pairs.contains(&(from.id.clone(), to.id.clone()))
            {
                let hit = Hit::declaration(edge.site.as_ref().unwrap_or(origin), &from.id, edge.site.is_none());
                index.usages.entry(to.id.clone()).or_default().push(hit);
            }
        }
        index
    }

    pub fn symbol(&self, id: &str) -> Option<&Symbol> {
        self.symbols.get(id).map(|&i| &self.graph.symbols[i])
    }

    pub fn hits(&self, symbol: &Symbol, kind: NavigationKind, state: Option<SemanticState>) -> (Coverage, Vec<Hit>) {
        if kind != NavigationKind::Declaration && state == Some(SemanticState::Loading) {
            // A provider updating a changed file can still have positions from its old analysis.
            return (Coverage::Loading, Vec::new());
        }
        let coverage = if kind == NavigationKind::Declaration || self.indexed.contains(&symbol.language) {
            Coverage::Indexed
        } else {
            Coverage::Partial
        };
        let hits = match kind {
            NavigationKind::Declaration => symbol
                .origins
                .iter()
                .map(|span| Hit::declaration(span, &symbol.id, false))
                .collect(),
            NavigationKind::Usages => self.usages.get(&symbol.id).cloned().unwrap_or_default(),
            NavigationKind::Implementations => {
                let mut hits = Vec::new();
                let mut pending = vec![symbol.id.as_str()];
                let mut seen = HashSet::from([symbol.id.as_str()]);
                // Roslyn verifies every class against its full interface list. Following base
                // classes here could reintroduce a derived class rejected for a conflict.
                let checked_interface = symbol.kind == SymbolKind::Interface
                    && symbol.language == Language::Csharp
                    && self.indexed.contains(&symbol.language);
                while let Some(id) = pending.pop() {
                    for (from, hit) in self.implementations.get(id).into_iter().flatten() {
                        if !checked_interface && seen.insert(from.as_str()) {
                            pending.push(from);
                        }
                        if from != &symbol.id && self.symbol(from).is_some_and(|s| self.is_concrete_implementation(s))
                        {
                            hits.push(hit.clone());
                        }
                    }
                }
                hits
            }
        };
        (coverage, hits)
    }

    fn is_concrete_implementation(&self, symbol: &Symbol) -> bool {
        if symbol.kind.is_contract() || symbol.flavors.iter().any(|flavor| flavor == "abstract") {
            return false;
        }
        let in_contract = symbol.parent.as_deref().and_then(|id| self.symbol(id)).is_some_and(|parent| parent.kind.is_contract());
        !in_contract || (symbol.language == Language::Csharp && (
            symbol.has_flow || (matches!(symbol.kind, SymbolKind::Property | SymbolKind::Event) &&
                self.graph.symbols.iter().any(|child| child.has_flow && child.parent.as_deref() == Some(symbol.id.as_str())))
        ))
    }
}

fn associated_trait_matches(member: &Symbol, target: &Symbol, unique_name: bool) -> bool {
    let Some(path) = member.flavors.iter().find_map(|flavor| flavor.strip_prefix("associated-trait-path:")) else {
        return unique_name;
    };
    if !path.contains("::") {
        return unique_name;
    }
    if let Some(path) = path.strip_prefix("crate::") {
        let owner_crate = member.id.split("::").next();
        return target.qualified_name.split_once("::")
            .is_some_and(|(target_crate, target_path)| Some(target_crate) == owner_crate && target_path == path);
    }
    target.qualified_name == path
}

pub struct NavigationQuery {
    pub(crate) coverage: Coverage,
    pub(crate) hits: Vec<Hit>,
    pub(crate) paths: HashMap<String, (PathBuf, Option<Stat>)>,
}

#[derive(Serialize)]
pub struct NavigationResult {
    coverage: Coverage,
    locations: Vec<Location>,
}

#[derive(Serialize)]
struct Location {
    span: Span,
    symbol: Option<String>,
    approximate: bool,
    snippet: String,
    source_revision: String,
}

/// Identifies the exact UTF-8 contents used to compute a navigation span.
pub fn source_revision(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

impl NavigationQuery {
    pub fn resolve(self) -> NavigationResult {
        let mut by_file: std::collections::BTreeMap<String, Vec<Hit>> = Default::default();
        for hit in self.hits {
            by_file.entry(hit.site.file.clone()).or_default().push(hit);
        }
        let mut locations = Vec::new();
        let mut coverage = self.coverage;
        for (file, mut hits) in by_file {
            let Some((path, stat)) = self.paths.get(&file) else {
                continue;
            };
            let text = match read_indexed_source(path, *stat) {
                Ok(text) => text,
                Err(error) if matches!(error.kind(), ErrorKind::NotFound | ErrorKind::WouldBlock) => {
                    return NavigationResult {
                        coverage: Coverage::Loading,
                        locations: Vec::new(),
                    };
                }
                Err(_) => {
                    coverage = Coverage::Partial;
                    continue;
                }
            };
            // Monaco excludes the BOM from the first line; semantic ranges include its bytes.
            let editor_text = text.strip_prefix('\u{feff}').unwrap_or(&text);
            let bom = text.len() - editor_text.len();
            let index = LineIndex::new(editor_text);
            let source_revision = source_revision(&text);
            hits.sort_by_key(|hit| (hit.site.start, hit.approximate, hit.site.end));
            hits.dedup_by_key(|hit| hit.site.start);
            for Hit {
                site,
                symbol,
                approximate,
            } in hits
            {
                let (start, end) = (site.start as usize, site.end as usize);
                if start > end || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                    continue;
                }
                let mut span = index.span(&file, start.saturating_sub(bom), end.saturating_sub(bom));
                span.start_byte = site.start;
                span.end_byte = site.end;
                let snippet = index
                    .line_text(span.start_line)
                    .unwrap_or("")
                    .trim()
                    .chars()
                    .take(240)
                    .collect();
                locations.push(Location {
                    span,
                    symbol,
                    approximate,
                    snippet,
                    source_revision: source_revision.clone(),
                });
            }
        }
        NavigationResult { coverage, locations }
    }
}

/// A changed or removed file requires a new index. An unreadable file only limits coverage.
fn read_indexed_source(path: &Path, indexed: Option<Stat>) -> io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    if Stat::of(&file.metadata()?) != indexed {
        return Err(ErrorKind::WouldBlock.into());
    }
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    if Stat::of(&std::fs::metadata(path)?) != indexed {
        return Err(ErrorKind::WouldBlock.into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_are_withheld_when_the_file_changes_before_the_watcher_refreshes() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let file = std::env::temp_dir().join(format!("nodex-navigation-{}-{unique}.ts", std::process::id()));
        std::fs::write(&file, "\u{feff}😀run();\n").unwrap();
        let stat = Stat::of(&std::fs::metadata(&file).unwrap());
        let query = || NavigationQuery {
            coverage: Coverage::Indexed,
            hits: vec![Hit {
                site: ByteRange {
                    file: "sample.ts".into(),
                    start: 7,
                    end: 10,
                },
                symbol: None,
                approximate: false,
            }],
            paths: HashMap::from([("sample.ts".into(), (file.clone(), stat))]),
        };
        let before = query().resolve();
        std::fs::write(&file, "// Inserted before the call.\nrun();\n").unwrap();
        let after = query().resolve();
        std::fs::remove_file(file).unwrap();
        assert!(matches!(before.coverage, Coverage::Indexed));
        assert_eq!(before.locations[0].snippet, "😀run();");
        assert_eq!(before.locations[0].span.start_byte, 7);
        assert_eq!(before.locations[0].span.start_col, 3);
        assert!(matches!(after.coverage, Coverage::Loading));
        assert!(after.locations.is_empty());
    }
}
