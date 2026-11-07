//! The project model: files on disk lowered by language frontends.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};
use nodex_ir::semantic::{SemanticFacts, SemanticState, SemanticStatus};
use nodex_ir::{Diagnostic, FileIr, FlowGraph, Language, PackageRef, StructEdgeKind, StructureGraph};
use nodex_lang_api::{LanguageFrontend, ProjectDescriptor, SourceFile};
use rayon::prelude::*;
use serde::Serialize;
use ts_rs::TS;

use crate::{link, semantic};

/// Directories never scanned (build output, dependencies, engine caches, VCS).
pub const IGNORED_DIRS: &[&str] = &[
    ".git", ".hg", ".svn", ".vs", ".idea", ".vscode", ".nodex", "bin", "obj", "node_modules", "target",
    "dist", "build", "Library", "Temp", "Logs", "UserSettings", "MemoryCaptures",
];

/// Files larger than this are skipped (generated code, minified bundles).
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct FileSummary {
    pub path: String,
    pub language: Language,
    pub symbols: u32,
    pub flows: u32,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Serialize, TS, Clone, Debug)]
#[ts(export)]
pub struct ProjectSummary {
    pub name: String,
    pub root: String,
    pub languages: Vec<Language>,
    pub flavors: Vec<String>,
    pub packages: Vec<PackageRef>,
    pub files: Vec<FileSummary>,
    /// Semantic analysis progress per language.
    pub semantic: Vec<SemanticStatus>,
}

struct FileEntry {
    language: Language,
    ir: FileIr,
}

pub struct Project {
    root: PathBuf,
    frontends: Vec<Arc<dyn LanguageFrontend>>,
    descriptors: Vec<ProjectDescriptor>,
    files: BTreeMap<String, FileEntry>,
    structure: Option<Arc<StructureGraph>>,
    semantic: BTreeMap<Language, SemanticEntry>,
    locator: Option<Arc<semantic::Locator>>,
}

struct SemanticEntry {
    status: SemanticStatus,
    facts: Option<Arc<SemanticFacts>>,
}

impl Project {
    /// Scans `root` and lowers every supported file in parallel.
    pub fn open(root: &Path, frontends: Vec<Arc<dyn LanguageFrontend>>) -> anyhow::Result<Self> {
        let root = dunce_canonical(root).with_context(|| format!("cannot open {}", root.display()))?;
        if !root.is_dir() {
            bail!("{} is not a directory", root.display());
        }
        let descriptors = frontends.iter().filter_map(|f| f.detect(&root)).collect();
        let mut project = Self {
            root,
            frontends,
            descriptors,
            files: BTreeMap::new(),
            structure: None,
            semantic: BTreeMap::new(),
            locator: None,
        };

        let paths: Vec<PathBuf> = walkdir::WalkDir::new(&project.root)
            .into_iter()
            .filter_entry(|e| !(e.file_type().is_dir() && e.depth() > 0 && is_ignored_dir(e.file_name().to_str().unwrap_or(""))))
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .filter(|p| project.frontend_for(p).is_some())
            .collect();

        let lowered: Vec<(String, FileEntry)> = paths
            .par_iter()
            .filter_map(|p| project.lower_path(p))
            .collect();
        project.files.extend(lowered);
        Ok(project)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn frontend_for(&self, path: &Path) -> Option<&Arc<dyn LanguageFrontend>> {
        let ext = path.extension()?.to_str()?;
        self.frontends.iter().find(|f| f.extensions().contains(&ext))
    }

    /// Converts an absolute path inside the root to a project-relative path with `/`.
    pub fn relative(&self, path: &Path) -> Option<String> {
        let rel = path.strip_prefix(&self.root).ok()?;
        let mut parts = Vec::new();
        for c in rel.components() {
            match c {
                Component::Normal(s) => parts.push(s.to_str()?.to_owned()),
                _ => return None,
            }
        }
        Some(parts.join("/"))
    }

    /// Resolves a project-relative path to an absolute one, refusing anything outside the root.
    pub fn absolute(&self, rel: &str) -> Option<PathBuf> {
        let mut path = self.root.clone();
        for part in rel.split('/') {
            if part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':']) {
                return None;
            }
            path.push(part);
        }
        Some(path)
    }

    /// True if `rel` lies under an ignored directory.
    pub fn is_ignored(rel: &str) -> bool {
        let parts: Vec<&str> = rel.split('/').collect();
        parts[..parts.len().saturating_sub(1)].iter().any(|p| is_ignored_dir(p))
    }

    fn lower_path(&self, path: &Path) -> Option<(String, FileEntry)> {
        let frontend = self.frontend_for(path)?;
        let rel = self.relative(path)?;
        if Self::is_ignored(&rel) {
            return None;
        }
        let meta = std::fs::metadata(path).ok()?;
        if meta.len() > MAX_FILE_BYTES {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let ir = frontend.lower_file(&SourceFile { rel_path: rel.clone(), text });
        Some((rel, FileEntry { language: frontend.id(), ir }))
    }

    /// Re-lowers (or removes) the file at `path` after a change on disk. Returns the relative
    /// path when the project changed.
    pub fn refresh_path(&mut self, path: &Path) -> Option<String> {
        let rel = self.relative(path)?;
        if Self::is_ignored(&rel) || self.frontend_for(path).is_none() {
            return None;
        }
        if path.is_file() {
            let (rel, entry) = self.lower_path(path)?;
            if self.files.get(&rel).is_some_and(|old| old.ir == entry.ir) {
                return None;
            }
            self.files.insert(rel.clone(), entry);
        } else if self.files.remove(&rel).is_none() {
            return None;
        }
        self.invalidate();
        Some(rel)
    }

    pub fn summary(&self) -> ProjectSummary {
        let mut languages: Vec<Language> = self.files.values().map(|f| f.language).collect();
        languages.sort();
        languages.dedup();
        let name = self
            .descriptors
            .first()
            .map(|d| d.name.clone())
            .or_else(|| self.root.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        ProjectSummary {
            name,
            root: self.root.display().to_string(),
            languages,
            flavors: self.descriptors.iter().flat_map(|d| d.flavors.clone()).collect(),
            packages: self.descriptors.iter().flat_map(|d| d.packages.clone()).collect(),
            semantic: self.semantic.values().map(|e| e.status.clone()).collect(),
            files: self
                .files
                .iter()
                .map(|(path, f)| FileSummary {
                    path: path.clone(),
                    language: f.language,
                    symbols: f.ir.symbols.len() as u32,
                    flows: f.ir.flows.len() as u32,
                    diagnostics: f.ir.diagnostics.clone(),
                })
                .collect(),
        }
    }

    /// The linked structure graph (cached until a file changes).
    pub fn structure(&mut self) -> Arc<StructureGraph> {
        if let Some(s) = &self.structure {
            return s.clone();
        }
        let files: Vec<&FileIr> = self.files.values().map(|f| &f.ir).collect();
        let packages: Vec<(Language, PackageRef)> = self
            .descriptors
            .iter()
            .flat_map(|d| d.packages.iter().map(move |p| (d.language, p.clone())))
            .collect();
        let mut graph = link::link(&files, &packages);

        // Semantic facts replace guesses: exact bases, calls, events.
        let locator = Arc::new(semantic::Locator::new(&graph.symbols));
        for (language, entry) in &self.semantic {
            let Some(facts) = &entry.facts else { continue };
            let derived = semantic::edges(facts, &locator, *language);
            graph.edges.retain(|e| {
                !(matches!(e.kind, StructEdgeKind::Inherits | StructEdgeKind::Implements) && derived.typed_bases.contains(&e.from))
            });
            for ext in derived.externals {
                if !graph.symbols.iter().any(|s| s.id == ext.id) {
                    graph.symbols.push(ext);
                }
            }
            for e in derived.edges {
                if !graph.edges.contains(&e) {
                    graph.edges.push(e);
                }
            }
        }
        // Drop external types no edge refers to any more (name-based guesses replaced above).
        let referenced: std::collections::HashSet<&str> =
            graph.edges.iter().flat_map(|e| [e.from.as_str(), e.to.as_str()]).collect();
        let orphans: Vec<String> = graph
            .symbols
            .iter()
            .filter(|s| s.id.starts_with("ext:") && !referenced.contains(s.id.as_str()))
            .map(|s| s.id.clone())
            .collect();
        graph.symbols.retain(|s| !orphans.contains(&s.id));

        let graph = Arc::new(graph);
        self.locator = Some(locator);
        self.structure = Some(graph.clone());
        graph
    }

    /// The flow graph of `symbol`, enriched with semantic facts when available.
    pub fn flow(&mut self, symbol: &str) -> Option<FlowGraph> {
        let (language, raw) = self
            .files
            .values()
            .find_map(|f| f.ir.flows.iter().find(|g| g.symbol == symbol).map(|g| (f.language, g.clone())))?;
        let Some(facts) = self.semantic.get(&language).and_then(|e| e.facts.clone()) else {
            return Some(raw);
        };
        if self.locator.is_none() {
            self.structure();
        }
        let locator = self.locator.clone()?;
        Some(semantic::enrich_flow(&raw, &facts, &locator))
    }

    /// Frontends for the languages present in the project.
    pub fn active_frontends(&self) -> Vec<Arc<dyn LanguageFrontend>> {
        let mut used: Vec<Language> = self.files.values().map(|f| f.language).collect();
        used.sort();
        used.dedup();
        self.frontends.iter().filter(|f| used.contains(&f.id())).cloned().collect()
    }

    /// Records progress of a semantic provider (loading, unavailable).
    pub fn set_semantic_status(&mut self, language: Language, state: SemanticState, detail: impl Into<String>) {
        let entry = self.semantic.entry(language).or_insert_with(|| SemanticEntry {
            status: SemanticStatus { language, state, detail: String::new(), messages: Vec::new() },
            facts: None,
        });
        entry.status.state = state;
        entry.status.detail = detail.into();
        if state == SemanticState::Unavailable {
            entry.facts = None;
            self.invalidate();
        }
    }

    /// Installs fresh facts for a language.
    pub fn apply_semantic(&mut self, language: Language, facts: SemanticFacts) {
        let status = SemanticStatus {
            language,
            state: SemanticState::Ready,
            detail: facts.source.clone(),
            messages: facts.messages.clone(),
        };
        self.semantic.insert(language, SemanticEntry { status, facts: Some(Arc::new(facts)) });
        self.invalidate();
    }

    fn invalidate(&mut self) {
        self.structure = None;
        self.locator = None;
    }

    /// Every flow graph of the project (syntax-only; node IDs and spans match enriched flows).
    pub fn flows(&self) -> impl Iterator<Item = &FlowGraph> {
        self.files.values().flat_map(|f| f.ir.flows.iter())
    }

    pub fn file_ir(&self, rel: &str) -> Option<&FileIr> {
        self.files.get(rel).map(|f| &f.ir)
    }

    /// Reads a project file. Only files the project tracks can be read.
    pub fn source(&self, rel: &str) -> Option<String> {
        if !self.files.contains_key(rel) {
            return None;
        }
        let path = self.absolute(rel)?;
        let bytes = std::fs::read(path).ok()?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

fn is_ignored_dir(name: &str) -> bool {
    IGNORED_DIRS.contains(&name)
}

/// Canonicalizes without the `\\?\` prefix Windows adds, so paths stay readable.
fn dunce_canonical(path: &Path) -> std::io::Result<PathBuf> {
    let canonical = path.canonicalize()?;
    #[cfg(windows)]
    {
        let s = canonical.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\")
            && !stripped.starts_with("UNC\\") {
                return Ok(PathBuf::from(stripped));
            }
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_directories_are_detected() {
        assert!(Project::is_ignored("obj/Debug/x.cs"));
        assert!(Project::is_ignored("Library/PackageCache/a/b.cs"));
        assert!(!Project::is_ignored("Assets/Scripts/Player.cs"));
        assert!(!Project::is_ignored("bin.cs"));
    }
}
