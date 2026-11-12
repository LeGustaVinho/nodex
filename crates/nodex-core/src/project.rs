//! The project model: files on disk lowered by language frontends.

use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};
use nodex_ir::knowledge::ProjectKnowledge;
use nodex_ir::semantic::{SemanticFacts, SemanticState, SemanticStatus};
use nodex_ir::{Diagnostic, FileIr, FlowGraph, Language, PackageRef, StructEdge, StructEdgeKind, StructureGraph, SymbolKind};
use nodex_lang_api::{LanguageFrontend, ProjectDescriptor, SourceFile};
use rayon::prelude::*;
use serde::Serialize;
use ts_rs::TS;

use crate::{cache, knowledge, link, semantic};

/// Directories never scanned (build output, dependencies, engine caches, VCS).
pub use nodex_lang_api::IGNORED_DIRS;
use nodex_lang_api::is_ignored_dir;

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
    /// False until a folder is opened (the desktop app starts on its welcome screen).
    pub open: bool,
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
    stat: Option<cache::Stat>,
}

/// A file lowered while opening, with what the lowering cache needs to keep it.
struct Lowered {
    rel: String,
    entry: FileEntry,
    blob: Blob,
}

enum Blob {
    /// Read back from the cache: its bytes are still in the loaded bundle.
    Cached,
    /// Lowered now and encoded for the cache.
    Fresh(Vec<u8>),
    /// Not for the cache (there is none, or the file did not encode).
    None,
}

pub struct Project {
    root: PathBuf,
    frontends: Vec<Arc<dyn LanguageFrontend>>,
    descriptors: Vec<ProjectDescriptor>,
    files: BTreeMap<String, Arc<FileEntry>>,
    semantic_configs: HashMap<String, Option<cache::Stat>>,
    structure: Option<Arc<StructureGraph>>,
    knowledge: Option<Arc<ProjectKnowledge>>,
    semantic: BTreeMap<Language, SemanticEntry>,
    locator: Option<Arc<semantic::Locator>>,
    navigation: Option<crate::navigation::NavigationIndex>,
    borrow_calls: Option<crate::borrow_origin::CallIndex>,
    /// Bumped whenever the structure graph must be built again (files, semantic facts).
    version: u64,
    /// Changes when tracked files change, independently of semantic provider results.
    source_revision: u64,
    /// Where each flow graph lives: symbol → (file, index in its flows). Built on first use.
    flow_index: Option<HashMap<String, (String, usize)>>,
    /// Files the open read back from the lowering cache instead of lowering them.
    from_cache: usize,
}

struct SemanticEntry {
    status: SemanticStatus,
    facts: Option<Arc<SemanticFacts>>,
    /// Indexes into `facts.refs` and `facts.locals`, by file, for enriching one flow.
    by_file: Arc<HashMap<String, SemanticFileFacts>>,
}

#[derive(Default)]
struct SemanticFileFacts {
    refs: Vec<usize>,
    locals: Vec<usize>,
    navigation: Vec<usize>,
    expressions: Vec<usize>,
    values: Vec<usize>,
    types: Vec<usize>,
    binding_uses: Vec<usize>,
    reading: Vec<usize>,
}

/// How far opening a project got, for a progress bar: `phase` is `scanning` (files found so
/// far, `total` unknown: 0) or `reading` (files lowered out of `total`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenProgress {
    pub phase: &'static str,
    pub done: usize,
    pub total: usize,
}

/// What building the structure graph and the knowledge needs: a snapshot taken under the
/// project lock in a moment, so the build itself ([`BuildInput::build`]) runs without it.
pub struct BuildInput {
    version: u64,
    root: PathBuf,
    files: Vec<Arc<FileEntry>>,
    descriptors: Vec<ProjectDescriptor>,
    semantic: Vec<(Language, Arc<SemanticFacts>)>,
}

/// A built structure graph, knowledge and locator, for [`Project::install_build`].
pub struct Built {
    version: u64,
    structure: Arc<StructureGraph>,
    knowledge: Arc<ProjectKnowledge>,
    locator: Arc<semantic::Locator>,
    navigation: crate::navigation::NavigationIndex,
    borrow_calls: crate::borrow_origin::CallIndex,
}

impl BuildInput {
    /// Links the files, applies the semantic facts and derives the knowledge.
    pub fn build(self) -> Built {
        let files: Vec<&FileIr> = self.files.iter().map(|f| &f.ir).collect();
        let packages: Vec<(Language, PackageRef)> = self
            .descriptors
            .iter()
            .flat_map(|d| d.packages.iter().map(move |p| (d.language, p.clone())))
            .collect();
        let total = crate::perf::phase("structure");
        let timer = crate::perf::phase("structure.link");
        let mut graph = link::link(&files, &packages);
        timer.finish(format_args!("({} symbols, {} edges)", graph.symbols.len(), graph.edges.len()));

        // Semantic facts replace guesses: exact bases, calls, events.
        let locator = Arc::new(semantic::Locator::new(&graph.symbols));
        let mut constructs = knowledge::Constructs::new();
        let mut calls: Vec<knowledge::RawCallSite> = Vec::new();
        let timer = crate::perf::phase("structure.semantic");
        let mut known: std::collections::HashSet<String> = graph.symbols.iter().map(|s| s.id.clone()).collect();
        let contracts: std::collections::HashSet<_> = self.semantic.iter()
            .flat_map(|(_, facts)| &facts.implementation_contracts)
            .filter_map(|site| locator.type_at(site))
            .collect();
        for symbol in &mut graph.symbols {
            if symbol.kind == SymbolKind::TypeAlias && contracts.contains(symbol.id.as_str())
                && !symbol.flavors.iter().any(|flavor| flavor == "object-contract")
            {
                symbol.flavors.push("object-contract".into());
            }
        }
        for (language, facts) in &self.semantic {
            semantic::annotate_member_values(&mut graph, &facts.values, &locator);
            semantic::annotate_member_types(&mut graph, &facts.types, &locator);
            let derived = semantic::edges(facts, &locator, *language);
            for (site, owner, target) in derived.calls {
                calls.push(knowledge::RawCallSite { file: site.file, start: site.start, end: site.end, owner, target });
            }
            for (site, ty) in derived.constructs {
                constructs.entry(site.file).or_default().push((site.start, site.end, ty));
            }
            graph.edges.retain(|e| {
                !(matches!(e.kind, StructEdgeKind::Inherits | StructEdgeKind::Implements) && derived.typed_bases.contains(&e.from))
            });
            for ext in derived.externals {
                if known.insert(ext.id.clone()) {
                    graph.symbols.push(ext);
                }
            }
            let mut present: std::collections::HashSet<StructEdge> = graph.edges.iter().cloned().collect();
            for e in derived.edges {
                if present.insert(e.clone()) {
                    graph.edges.push(e);
                }
            }
        }
        timer.finish(format_args!("({} languages with facts)", self.semantic.len()));
        // Knowledge: routes, HTTP calls, DI, tests, throws, complexity.
        let package_roots: Vec<String> = self
            .descriptors
            .iter()
            .flat_map(|d| d.manifests.iter())
            .filter_map(|m| m.parent().and_then(|dir| relative_to(&self.root, dir)))
            .collect();
        let timer = crate::perf::phase("structure.knowledge");
        let derived = knowledge::build(&graph, &files, &constructs, &calls, &package_roots);
        timer.finish("");
        let timer = crate::perf::phase("structure.merge");
        for ext in derived.externals {
            if known.insert(ext.id.clone()) {
                graph.symbols.push(ext);
            }
        }
        // Exact relations already in the graph win over a derived copy (same endpoints and kind).
        let mut present: std::collections::HashSet<(String, String, StructEdgeKind)> =
            graph.edges.iter().map(|e| (e.from.clone(), e.to.clone(), e.kind)).collect();
        for e in derived.edges {
            if present.insert((e.from.clone(), e.to.clone(), e.kind)) {
                graph.edges.push(e);
            }
        }

        // Drop external types no edge refers to any more (name-based guesses replaced above).
        let referenced: std::collections::HashSet<&str> =
            graph.edges.iter().flat_map(|e| [e.from.as_str(), e.to.as_str()]).collect();
        let orphans: std::collections::HashSet<String> = graph
            .symbols
            .iter()
            .filter(|s| s.id.starts_with("ext:") && !referenced.contains(s.id.as_str()))
            .map(|s| s.id.clone())
            .collect();
        graph.symbols.retain(|s| !orphans.contains(&s.id));
        timer.finish("");
        total.finish(format_args!("({} symbols, {} edges)", graph.symbols.len(), graph.edges.len()));

        let structure = Arc::new(graph);
        let navigation = crate::navigation::NavigationIndex::new(structure.clone(), &locator, &self.semantic);
        let nominal_types = self.semantic.iter().filter(|(language, _)| *language == Language::Rust)
            .flat_map(|(_, facts)| &facts.rust_nominal_types);
        let borrow_facts = self.semantic.iter().filter(|(language, _)| *language == Language::Rust)
            .flat_map(|(_, facts)| &facts.rust_borrows);
        let borrow_calls = crate::borrow_origin::CallIndex::new(&calls, &structure, &locator, nominal_types, borrow_facts);
        Built { version: self.version, structure, knowledge: Arc::new(derived.project), locator, navigation, borrow_calls }
    }

}

impl Project {
    /// Scans `root` and lowers every supported file in parallel.
    pub fn open(root: &Path, frontends: Vec<Arc<dyn LanguageFrontend>>) -> anyhow::Result<Self> {
        Self::open_with_progress(root, frontends, &|_| {})
    }

    /// [`Project::open`], reporting progress about fifty times along the way.
    pub fn open_with_progress(
        root: &Path,
        frontends: Vec<Arc<dyn LanguageFrontend>>,
        progress: &(dyn Fn(OpenProgress) + Sync),
    ) -> anyhow::Result<Self> {
        Self::open_cached(root, frontends, None, progress)
    }

    /// [`Project::open_with_progress`] with a lowering cache under `cache` (see [`cache`]):
    /// files whose size and modification time match the last open are read back instead of
    /// lowered, and the cache is brought up to date afterwards.
    pub fn open_cached(
        root: &Path,
        frontends: Vec<Arc<dyn LanguageFrontend>>,
        cache: Option<&Path>,
        progress: &(dyn Fn(OpenProgress) + Sync),
    ) -> anyhow::Result<Self> {
        let root = dunce_canonical(root).with_context(|| format!("cannot open {}", root.display()))?;
        if !root.is_dir() {
            bail!("{} is not a directory", root.display());
        }
        let mut project = Self {
            root,
            frontends,
            descriptors: Vec::new(),
            files: BTreeMap::new(),
            structure: None,
            knowledge: None,
            semantic: BTreeMap::new(),
            locator: None,
            navigation: None,
            borrow_calls: None,
            version: 0,
            source_revision: 0,
            semantic_configs: HashMap::new(),
            flow_index: None,
            from_cache: 0,
        };
        // The cache is read while the walk runs: both mostly wait on the disk.
        let (paths, cached) = rayon::join(
            || {
                let timer = crate::perf::phase("open.walk");
                let paths = project.source_paths(progress);
                timer.finish(format_args!("({} files)", paths.len()));
                paths
            },
            || {
                cache.map(|base| {
                    let timer = crate::perf::phase("open.cache.load");
                    let cached = cache::LowerCache::load(base, &project.root);
                    timer.finish(format_args!("({} files)", cached.len()));
                    cached
                })
            },
        );
        // Detection (each language walks the top of the tree for its manifests) runs only for the
        // languages that have files, and at the same time as lowering: neither needs the other.
        let present: std::collections::HashSet<Language> =
            paths.iter().filter_map(|p| project.frontend_for(p)).map(|f| f.id()).collect();
        let (descriptors, lowered) = rayon::join(
            || {
                let timer = crate::perf::phase("open.detect");
                let found: Vec<ProjectDescriptor> = project
                    .frontends
                    .par_iter()
                    .filter(|f| present.contains(&f.id()))
                    .filter_map(|f| f.detect(&project.root))
                    .collect();
                timer.finish(format_args!("({} descriptors)", found.len()));
                found
            },
            || project.lower_paths(&paths, cached.as_ref(), progress),
        );
        project.descriptors = descriptors;
        project.from_cache = lowered.iter().filter(|l| matches!(l.blob, Blob::Cached)).count();
        if let Some(cached) = &cached {
            // Rewrite the bundle only when something changed: a file lowered now, or one gone.
            let fresh = lowered.iter().any(|l| matches!(l.blob, Blob::Fresh(_)));
            if fresh || project.from_cache != cached.len() {
                let timer = crate::perf::phase("open.cache.save");
                cached.save(lowered.iter().filter_map(|l| {
                    let stat = l.entry.stat?;
                    let bytes = match &l.blob {
                        Blob::Cached => cached.bytes(&l.rel, stat)?,
                        Blob::Fresh(bytes) => bytes,
                        Blob::None => return None,
                    };
                    Some(cache::Entry { rel: &l.rel, stat, bytes })
                }));
                timer.finish("");
            }
        }
        project.files.extend(lowered.into_iter().map(|l| (l.rel, Arc::new(l.entry))));
        Ok(project)
    }

    /// Files the open read back from the lowering cache (0 without a cache).
    pub fn files_from_cache(&self) -> usize {
        self.from_cache
    }

    /// Lowers `paths` in parallel, reading unchanged files back from `cache`.
    fn lower_paths(&self, paths: &[PathBuf], cache: Option<&cache::LowerCache>, progress: &(dyn Fn(OpenProgress) + Sync)) -> Vec<Lowered> {
        let timer = crate::perf::phase("open.lower");
        let total = paths.len();
        progress(OpenProgress { phase: "reading", done: 0, total });
        // About fifty updates whatever the size, from whichever thread reaches each step.
        let step = total.div_ceil(50).max(1);
        let done = std::sync::atomic::AtomicUsize::new(0);
        let lowered: Vec<Lowered> = paths
            .par_iter()
            .filter_map(|p| {
                let lowered = self.lower_path_cached(p, cache);
                let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if n % step == 0 || n == total {
                    progress(OpenProgress { phase: "reading", done: n, total });
                }
                lowered
            })
            .collect();
        let cached = lowered.iter().filter(|l| matches!(l.blob, Blob::Cached)).count();
        timer.finish(format_args!("({} files, {} lowered, {cached} from the cache)", lowered.len(), lowered.len() - cached));
        lowered
    }

    /// Every file a frontend reads, found by walking the root on several threads. Skips
    /// [`IGNORED_DIRS`] and what the project's own `.gitignore`, `.ignore` and `.git/info/exclude`
    /// leave out (not ignore files above the root: a project inside a folder its enclosing
    /// repository ignores must still open). Sorted, so the result does not depend on threads.
    fn source_paths(&self, progress: &(dyn Fn(OpenProgress) + Sync)) -> Vec<PathBuf> {
        let found = std::sync::Mutex::new(Vec::new());
        let count = std::sync::atomic::AtomicUsize::new(0);
        ignore::WalkBuilder::new(&self.root)
            .hidden(false)
            .parents(false)
            .git_global(false)
            .filter_entry(|e| !(e.depth() > 0 && e.file_type().is_some_and(|t| t.is_dir()) && is_ignored_dir(e.file_name().to_str().unwrap_or(""))))
            .build_parallel()
            .run(|| {
                let (found, count) = (&found, &count);
                Box::new(move |entry| {
                    if let Ok(entry) = entry
                        && entry.file_type().is_some_and(|t| t.is_file())
                        && self.frontend_for(entry.path()).is_some()
                    {
                        if let Ok(mut f) = found.lock() {
                            f.push(entry.into_path());
                        }
                        let n = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        if n % 1000 == 0 {
                            progress(OpenProgress { phase: "scanning", done: n, total: 0 });
                        }
                    }
                    ignore::WalkState::Continue
                })
            });
        let mut paths = found.into_inner().unwrap_or_default();
        paths.sort();
        paths
    }

    /// No folder yet: an empty project whose root is empty. The server starts with it when it
    /// is launched without a folder; every graph is empty until one is opened.
    pub fn empty(frontends: Vec<Arc<dyn LanguageFrontend>>) -> Self {
        Self {
            root: PathBuf::new(),
            frontends,
            descriptors: Vec::new(),
            files: BTreeMap::new(),
            structure: None,
            knowledge: None,
            semantic: BTreeMap::new(),
            locator: None,
            navigation: None,
            borrow_calls: None,
            version: 0,
            source_revision: 0,
            semantic_configs: HashMap::new(),
            flow_index: None,
            from_cache: 0,
        }
    }

    /// True once a folder is open (false for [`Project::empty`]).
    pub fn is_open(&self) -> bool {
        !self.root.as_os_str().is_empty()
    }

    /// Every language frontend the project was opened with.
    pub fn frontends(&self) -> Vec<Arc<dyn LanguageFrontend>> {
        self.frontends.clone()
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
        relative_to(&self.root, path)
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
        self.lower_path_cached(path, None).map(|l| (l.rel, l.entry))
    }

    /// Lowers the file at `path`, or reads it back from `cache` when it has not changed.
    fn lower_path_cached(&self, path: &Path, cache: Option<&cache::LowerCache>) -> Option<Lowered> {
        let frontend = self.frontend_for(path)?;
        let rel = self.relative(path)?;
        if Self::is_ignored(&rel) {
            return None;
        }
        let meta = std::fs::metadata(path).ok()?;
        if meta.len() > MAX_FILE_BYTES {
            return None;
        }
        let language = frontend.id();
        // Stat before reading: a change made while reading leaves a newer time on disk, so
        // the next open lowers the file again.
        let stat = cache::Stat::of(&meta);
        if let (Some(cache), Some(stat)) = (cache, stat)
            && let Some(ir) = cache.get(&rel, stat)
        {
            return Some(Lowered { rel, entry: FileEntry { language, ir, stat: Some(stat) }, blob: Blob::Cached });
        }
        let bytes = std::fs::read(path).ok()?;
        let source = SourceFile { rel_path: rel.clone(), text: String::from_utf8_lossy(&bytes).into_owned() };
        let mut ir = frontend.lower_file(&source);
        if !ir.flows.is_empty() || ir.symbols.iter().any(|s| s.syntax_sites.is_some()) {
            let revision = crate::navigation::source_revision(&source.text);
            ir.source_revision = Some(revision.clone());
            for flow in &mut ir.flows {
                flow.source_revision = Some(revision.clone());
            }
        }
        let blob = match cache.and(stat).and_then(|_| cache::encode(&ir)) {
            Some(bytes) => Blob::Fresh(bytes),
            None => Blob::None,
        };
        Some(Lowered { rel, entry: FileEntry { language, ir, stat }, blob })
    }

    /// Refreshes a source file or records a compiler configuration change on disk.
    pub fn refresh_path(&mut self, path: &Path) -> Option<String> {
        let rel = self.relative(path)?;
        if Self::is_ignored(&rel) {
            return None;
        }
        if self.frontend_for(path).is_none() {
            let declared_config = self.semantic.values().filter_map(|entry| entry.facts.as_ref())
                .any(|facts| facts.configuration_files.contains(&rel));
            let known_config = compiler_configuration_language(path)
                .is_some_and(|language| self.files.values().any(|file| file.language == language));
            if !declared_config && !known_config {
                return None;
            }
            let stat = std::fs::metadata(path).ok().filter(|meta| meta.is_file()).and_then(|meta| cache::Stat::of(&meta));
            if self.semantic_configs.insert(rel.clone(), stat) == Some(stat) {
                return None;
            }
        } else if path.is_file() {
            let (rel, entry) = self.lower_path(path)?;
            if self.files.get(&rel).is_some_and(|old| old.stat == entry.stat && old.ir == entry.ir) {
                return None;
            }
            self.files.insert(rel.clone(), Arc::new(entry));
        } else if self.files.remove(&rel).is_none() {
            return None;
        }
        self.source_revision += 1;
        for entry in self.semantic.values_mut().filter(|entry| entry.status.state == SemanticState::Ready) {
            entry.status.state = SemanticState::Loading;
            entry.status.detail = "updating semantic analysis…".into();
        }
        self.invalidate();
        Some(rel)
    }

    /// Display name: the first manifest's (`.csproj`, `package.json`, crate), else the folder's.
    pub fn name(&self) -> String {
        self.descriptors
            .first()
            .map(|d| d.name.clone())
            .or_else(|| self.root.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default()
    }

    /// The languages of the project's files, sorted.
    pub fn languages(&self) -> Vec<Language> {
        let mut languages: Vec<Language> = self.files.values().map(|f| f.language).collect();
        languages.sort();
        languages.dedup();
        languages
    }

    pub fn summary(&self) -> ProjectSummary {
        ProjectSummary {
            open: self.is_open(),
            name: self.name(),
            languages: self.languages(),
            root: self.root.display().to_string(),
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

    /// The linked structure graph (cached until a file changes). Builds it here when needed;
    /// a server takes [`Project::build_input`] instead and builds without holding the project.
    pub fn structure(&mut self) -> Arc<StructureGraph> {
        if let Some(s) = &self.structure {
            return s.clone();
        }
        let built = self.build_input().build();
        let graph = built.structure.clone();
        self.install_build(built);
        graph
    }

    /// The structure graph and knowledge, when they are current.
    pub fn built(&self) -> Option<(Arc<StructureGraph>, Arc<ProjectKnowledge>)> {
        Some((self.structure.clone()?, self.knowledge.clone()?))
    }

    /// Changes each time the structure graph must be built again.
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn source_revision(&self) -> u64 {
        self.source_revision
    }

    /// A snapshot of what building the graphs needs (cheap: shared file entries).
    pub fn build_input(&self) -> BuildInput {
        BuildInput {
            version: self.version,
            root: self.root.clone(),
            files: self.files.values().cloned().collect(),
            descriptors: self.descriptors.clone(),
            semantic: self.semantic.iter().filter_map(|(l, e)| Some((*l, e.facts.clone()?))).collect(),
        }
    }

    /// Keeps a build if nothing changed since its input was taken; false when it is stale.
    pub fn install_build(&mut self, built: Built) -> bool {
        if built.version != self.version {
            return false;
        }
        self.structure = Some(built.structure);
        self.knowledge = Some(built.knowledge);
        self.locator = Some(built.locator);
        self.navigation = Some(built.navigation);
        self.borrow_calls = Some(built.borrow_calls);
        true
    }

    /// Captures the current navigation locations and tracked paths without reading source files.
    pub fn navigation(&self, id: &str, kind: crate::navigation::NavigationKind) -> Option<crate::navigation::NavigationQuery> {
        let navigation = self.navigation.as_ref()?;
        let symbol = navigation.symbol(id).filter(|s| !s.external && !s.origins.is_empty())?;
        let state = self.semantic.get(&symbol.language).map(|e| e.status.state);
        let (coverage, hits) = navigation.hits(symbol, kind, state);
        let paths = hits.iter().map(|hit| &hit.site.file).collect::<std::collections::HashSet<_>>()
            .into_iter().filter_map(|file| {
                let entry = self.files.get(file)?;
                Some((file.clone(), (self.absolute(file)?, entry.stat)))
            }).collect();
        Some(crate::navigation::NavigationQuery { coverage, hits, paths })
    }

    /// Project-wide knowledge: routes, HTTP calls, DI, configuration, data models and per-symbol
    /// metrics (computed with the structure graph).
    pub fn knowledge(&mut self) -> Arc<ProjectKnowledge> {
        self.structure();
        self.knowledge.clone().unwrap_or_default()
    }

    fn ensure_flow_index(&mut self) {
        self.flow_index.get_or_insert_with(|| {
            let mut index = HashMap::new();
            for (rel, f) in &self.files {
                for (i, g) in f.ir.flows.iter().enumerate() {
                    index.entry(g.symbol.clone()).or_insert_with(|| (rel.clone(), i));
                }
            }
            index
        });
    }

    pub(crate) fn borrow_model(&self, symbol: &str) -> Option<(&FlowGraph, &nodex_ir::borrow::BorrowModel)> {
        let (rel, i) = self.flow_index.as_ref()?.get(symbol)?;
        let entry = self.files.get(rel)?;
        Some((entry.ir.flows.get(*i)?, entry.ir.borrow_models.get(symbol)?))
    }

    pub(crate) fn borrow_binding_type(&self, span: &nodex_ir::Span) -> Option<nodex_ir::ValueInfo> {
        use nodex_ir::semantic::ValueFactTarget;
        let entry = self.semantic.get(&Language::Rust)?;
        let facts = entry.facts.as_ref()?;
        let file = entry.by_file.get(&span.file)?;
        let mut at_site = file.values.iter().map(|&i| &facts.values[i]).filter(|value|
            value.target == ValueFactTarget::Binding && value.site.start == span.start_byte && value.site.end == span.end_byte);
        if let Some(first) = at_site.next() {
            return at_site.all(|other| other.info == first.info).then_some(first.info);
        }
        let local = file.locals.iter().map(|&i| &facts.locals[i]).find(|local|
            local.site.start == span.start_byte && local.site.end == span.end_byte)?;
        nodex_lang_api::value_shapes::written_type(Language::Rust, &local.ty)
    }

    pub fn borrow_origin(&mut self, flow: &str, node: &str, pin: Option<&str>, expr_path: Option<&str>, offset: usize)
        -> Option<crate::borrow_origin::BorrowOrigins>
    {
        self.structure();
        self.ensure_flow_index();
        crate::borrow_origin::resolve(self, self.borrow_calls.as_ref()?, flow, node, pin, expr_path, offset)
    }

    /// The flow graph of `symbol`, enriched with semantic facts when available.
    pub fn flow(&mut self, symbol: &str) -> Option<FlowGraph> {
        self.ensure_flow_index();
        let (rel, i) = self.flow_index.as_ref()?.get(symbol)?;
        let entry = self.files.get(rel)?;
        let (language, raw) = (entry.language, entry.ir.flows.get(*i)?.clone());
        let Some((facts, by_file)) = self.semantic.get(&language)
            .filter(|entry| entry.status.state == SemanticState::Ready)
            .and_then(|entry| Some((entry.facts.clone()?, entry.by_file.clone()))) else {
    pub(crate) fn rust_semantics_ready(&self) -> bool {
        self.semantic.get(&Language::Rust).is_some_and(|entry| entry.status.state == SemanticState::Ready)
    }

            let mut raw = raw;
            crate::operations::recognize_flow(&mut raw);
            return Some(raw);
        };
        if self.locator.is_none() {
            self.structure();
        }
        let locator = self.locator.clone()?;
        let file = by_file.get(&raw.span.file);
        let refs: Vec<_> = file.into_iter().flat_map(|f| &f.refs).map(|&i| &facts.refs[i]).collect();
        let locals: Vec<_> = file.into_iter().flat_map(|f| &f.locals).map(|&i| &facts.locals[i]).collect();
            if language == Language::Rust {
                let empty = crate::borrow_origin::CallIndex::default();
                crate::borrow_origin::attach_previews(self, &empty, &mut raw);
            }
        let mut flow = semantic::enrich_flow_with(&raw, &refs, &locals, &locator);
        if let Some(file) = file {
            semantic::annotate_expression_facts(&mut flow, file.expressions.iter().map(|&i| &facts.expressions[i]));
            semantic::annotate_value_facts(&mut flow, file.values.iter().map(|&i| &facts.values[i]), &locals);
            semantic::annotate_types(&mut flow, file.types.iter().map(|&i| &facts.types[i]), &locals);
        }
        if let (Some(file), Some(navigation)) = (file, &facts.navigation) {
            semantic::annotate_navigation(&mut flow, file.navigation.iter().map(|&i| &navigation[i]), &locator);
        }
        crate::operations::recognize_flow(&mut flow);
        let binding_uses = file.into_iter().flat_map(|file| &file.binding_uses).map(|&i| &facts.binding_uses[i]);
        semantic::annotate_collection_roles(&mut flow, &refs, binding_uses);
        Some(flow)
    }
            flow.attach_reading(file.reading.iter().map(|&i| &facts.reading[i]));

    /// Build units (`.csproj` projects, `package.json` packages, Cargo crates) and the
    /// references between them.
    pub fn units(&self) -> Vec<crate::units::BuildUnit> {
        let manifests: Vec<(Language, PathBuf, Vec<PackageRef>)> = self
            .descriptors
            .iter()
        if let Some(calls) = &self.borrow_calls
            && language == Language::Rust
        {
            crate::borrow_origin::attach_previews(self, calls, &mut flow);
        }
            .flat_map(|d| {
                // Packages are known per language; give them to the unit only when it is the only one.
                let units = d.manifests.iter().filter(|m| is_unit_manifest(m)).count();
                d.manifests.iter().map(move |m| (d.language, m.clone(), if units == 1 { d.packages.clone() } else { Vec::new() }))
            })
            .collect();
        crate::units::read_units(&self.root, &manifests)
    }

    /// Frontends for the languages present in the project.
    pub fn active_frontends(&self) -> Vec<Arc<dyn LanguageFrontend>> {
        let mut used: Vec<Language> = self.files.values().map(|f| f.language).collect();
        used.sort();
        used.dedup();
        self.frontends.iter().filter(|f| used.contains(&f.id())).cloned().collect()
    }

    /// Records progress of a semantic provider (loading, unavailable). Returns true when the
    /// structure graph changed (facts that were in use are dropped).
    pub fn set_semantic_status(&mut self, language: Language, state: SemanticState, detail: impl Into<String>) -> bool {
        let entry = self.semantic.entry(language).or_insert_with(|| SemanticEntry {
            status: SemanticStatus { language, state, detail: String::new(), messages: Vec::new() },
            facts: None,
            by_file: Arc::default(),
        });
        entry.status.state = state;
        entry.status.detail = detail.into();
        if state == SemanticState::Unavailable && entry.facts.take().is_some() {
            entry.by_file = Arc::default();
            self.invalidate();
            return true;
        }
        false
    }

    /// Installs fresh facts for a language.
    pub fn apply_semantic(&mut self, language: Language, facts: SemanticFacts) {
        let status = SemanticStatus {
            language,
            state: SemanticState::Ready,
            detail: facts.source.clone(),
            messages: facts.messages.clone(),
        };
        let mut by_file: HashMap<String, SemanticFileFacts> = HashMap::new();
        for (i, r) in facts.refs.iter().enumerate() {
            by_file.entry(r.site.file.clone()).or_default().refs.push(i);
        }
        for (i, l) in facts.locals.iter().enumerate() {
            by_file.entry(l.site.file.clone()).or_default().locals.push(i);
        }
        for (i, value) in facts.values.iter().enumerate() {
            by_file.entry(value.site.file.clone()).or_default().values.push(i);
        }
        for (i, fact) in facts.types.iter().enumerate() {
            by_file.entry(fact.site.file.clone()).or_default().types.push(i);
        }
        for (i, fact) in facts.binding_uses.iter().enumerate() {
            by_file.entry(fact.declaration.file.clone()).or_default().binding_uses.push(i);
        }
        for (i, expression) in facts.expressions.iter().enumerate() {
            by_file.entry(expression.site.file.clone()).or_default().expressions.push(i);
        }
        for (i, fact) in facts.navigation.iter().flatten().enumerate().filter(|(_, fact)| !fact.implementation) {
            by_file.entry(fact.site.file.clone()).or_default().navigation.push(i);
        }
        self.semantic.insert(language, SemanticEntry { status, facts: Some(Arc::new(facts)), by_file: Arc::new(by_file) });
        self.invalidate();
    }
        for (i, fact) in facts.reading.iter().enumerate() {
            by_file.entry(fact.anchor.file.clone()).or_default().reading.push(i);
        }

    fn invalidate(&mut self) {
        self.structure = None;
        self.knowledge = None;
        self.locator = None;
        self.navigation = None;
        self.borrow_calls = None;
        self.flow_index = None;
        self.version += 1;
    }

    /// Every flow graph of the project (syntax-only; node IDs and spans match enriched flows).
    pub fn flows(&self) -> impl Iterator<Item = &FlowGraph> {
        self.files.values().flat_map(|f| f.ir.flows.iter())
    }

    pub fn file_ir(&self, rel: &str) -> Option<&FileIr> {
        self.files.get(rel).map(|f| &f.ir)
    }

    /// Every lowered file, by project-relative path.
    pub fn files(&self) -> impl Iterator<Item = (&str, &FileIr)> {
        self.files.iter().map(|(rel, f)| (rel.as_str(), &f.ir))
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

fn is_unit_manifest(path: &Path) -> bool {
    let name = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    name.ends_with(".csproj") || name == "package.json" || name == "Cargo.toml"
}

fn compiler_configuration_language(path: &Path) -> Option<Language> {
    let name = path.file_name().and_then(|f| f.to_str()).unwrap_or_default();
    if name == "package.json" || ["tsconfig.", "jsconfig."].iter().any(|prefix| name.starts_with(prefix) && name.ends_with(".json")) {
        return Some(Language::Typescript);
    }
    let extension = path.extension().and_then(|value| value.to_str())?.to_ascii_lowercase();
    matches!(extension.as_str(), "csproj" | "sln" | "slnx" | "props" | "targets").then_some(Language::Csharp)
}

/// `path` relative to `root`, with `/`.
fn relative_to(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_str()?.to_owned()),
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

/// Canonicalizes without the `\\?\` prefix Windows adds, so paths stay readable.
pub(crate) fn dunce_canonical(path: &Path) -> std::io::Result<PathBuf> {
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
