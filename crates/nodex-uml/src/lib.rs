//! # Nodex UML
//!
//! UML diagrams of a project — class, package, component, activity and sequence diagrams so far —
//! built from the structure graph, the knowledge graph and the flow graphs, and written as
//! [Mermaid](https://mermaid.js.org) text.
//!
//! Every diagram is built in two steps: a model of what to show (which types, packages or
//! components, and which relations, each with its evidence and confidence), then the Mermaid
//! text. Element ids in the text are synthetic (`n0`, `n1`, ...) and [`UmlDiagram::elements`]
//! maps them back to symbols, so the web view can link the drawing to the code.
//!
//! TypeScript bindings are generated with `ts-rs` into `web/src/gen` by `cargo test -p nodex-uml`.

mod activity;
mod availability;
mod calls;
mod class;
mod component;
mod flowtree;
mod index;
mod mermaid;
mod package;
mod roots;
mod scope;
mod sequence;
mod state;
mod types;
mod usecase;

use nodex_core::units::BuildUnit;
use nodex_ir::knowledge::{Confidence, ProjectKnowledge};
use nodex_ir::{FlowGraph, Span, StructureGraph, SymbolId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use availability::availability;
pub use index::Index;
pub use scope::Scoped;

/// The diagram kinds Nodex knows.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[ts(export)]
pub enum UmlKind {
    UseCase,
    Component,
    Class,
    Sequence,
    Activity,
    State,
    Package,
}

impl UmlKind {
    pub const ALL: [UmlKind; 7] =
        [UmlKind::UseCase, UmlKind::Component, UmlKind::Class, UmlKind::Sequence, UmlKind::Activity, UmlKind::State, UmlKind::Package];
}

/// What a diagram is about.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(tag = "kind")]
#[ts(export)]
pub enum UmlScope {
    /// The whole project.
    Project,
    /// Every file under a project-relative folder.
    Folder { path: String },
    /// One project-relative file.
    File { path: String },
    /// Selected code: byte range of one file.
    Range { file: String, start_byte: u32, end_byte: u32 },
    /// A symbol and what it contains (also `dir:<path>` and `grp:ns:<name>` containers).
    Symbol { id: SymbolId },
    /// Several symbols.
    Symbols { ids: Vec<SymbolId> },
}

/// Which members a class shows.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[ts(export)]
pub enum MemberFilter {
    /// Public members for a project or folder, every member for a file, a symbol or a selection.
    #[default]
    Auto,
    All,
    Public,
    None,
}

/// How a package diagram groups code.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[ts(export)]
pub enum PackageGrouping {
    /// C# namespaces, Rust modules, TypeScript folders.
    #[default]
    Namespace,
    /// Folders on disk.
    Folder,
}

/// How much an activity diagram shows.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[ts(export)]
pub enum Granularity {
    /// Straight runs of statements merged into one action (up to three lines).
    #[default]
    Grouped,
    /// One action per statement.
    Statement,
    /// Only decisions, loops and exits; each straight run becomes one action.
    Decisions,
}

/// Options shared by the diagrams (each uses the ones that apply).
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(default)]
#[ts(export)]
pub struct UmlOptions {
    /// Show external types and packages (`MonoBehaviour`, NuGet packages) as context.
    pub include_external: bool,
    /// Show the types just outside the scope that the scope's types relate to (dimmed).
    pub context_neighbors: bool,
    pub members: MemberFilter,
    /// Leave test code out.
    pub hide_tests: bool,
    /// Relations shown, by confidence (empty: all).
    pub confidences: Vec<Confidence>,
    pub grouping: PackageGrouping,
    /// Most elements drawn; the least relevant are summarized past it (0: the diagram's default).
    pub max_elements: u32,
    /// The function an activity or sequence diagram starts from (a candidate id); absent: the
    /// most relevant one in the scope.
    pub focus: Option<String>,
    /// How deep calls are followed: into callees (sequence, default 3) or expanded in place
    /// (activity, default 0).
    pub depth: Option<u32>,
    pub granularity: Granularity,
    /// Activity labels show the code instead of the plain-language description.
    pub code_labels: bool,
    /// Leave out logging, framework helpers and calls to libraries that are not infrastructure.
    pub hide_noise: bool,
    /// Sequence diagrams: draw the calls a recorded run made instead of reading the code.
    pub runtime: bool,
}

impl Default for UmlOptions {
    fn default() -> Self {
        Self {
            include_external: false,
            context_neighbors: true,
            members: MemberFilter::Auto,
            hide_tests: true,
            confidences: Vec::new(),
            grouping: PackageGrouping::Namespace,
            max_elements: 0,
            focus: None,
            depth: None,
            granularity: Granularity::Grouped,
            code_labels: false,
            hide_noise: true,
            runtime: false,
        }
    }
}

impl UmlOptions {
    pub(crate) fn allows(&self, c: Option<Confidence>) -> bool {
        self.confidences.is_empty() || self.confidences.contains(&c.unwrap_or(Confidence::Exact))
    }

    pub(crate) fn limit(&self, default: usize) -> usize {
        if self.max_elements == 0 { default } else { self.max_elements as usize }
    }
}

/// One drawn element (a class, a package, a component, an interface, an infrastructure node).
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct UmlElement {
    /// Id in the Mermaid text (`n3`).
    pub mermaid_id: String,
    /// Project symbol, when the element is one.
    pub symbol: Option<SymbolId>,
    pub label: String,
    /// `class`, `interface`, `package`, `component`, `database`, `external`, ...
    pub kind: String,
    /// Where it is declared.
    pub span: Option<Span>,
    /// Outside the scope, drawn for context.
    pub context: bool,
    /// Why it is drawn, in plain words (metrics of a package, the folder of a component).
    pub detail: Option<String>,
}

/// One drawn relation, with its evidence.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct UmlRelation {
    pub from: String,
    pub to: String,
    /// `inherits`, `realizes`, `composition`, `aggregation`, `association`, `dependency`, `http`, ...
    pub kind: String,
    pub label: Option<String>,
    /// Absent: exact.
    pub confidence: Option<Confidence>,
    pub evidence: Option<String>,
    pub site: Option<Span>,
    /// Underlying relations merged into this one.
    pub count: u32,
}

/// A choice the scope leaves open (several functions, several state variables).
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct UmlCandidate {
    pub id: String,
    pub label: String,
    pub detail: Option<String>,
    pub score: f32,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Default)]
#[ts(export)]
pub struct UmlStats {
    pub elements: u32,
    pub relations: u32,
    /// Elements left out (past the limit, or filtered).
    pub hidden: u32,
}

/// A diagram ready to render.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct UmlDiagram {
    pub kind: UmlKind,
    /// Plain-language name of the scope (`Folder Services`).
    pub title: String,
    pub mermaid: String,
    pub elements: Vec<UmlElement>,
    pub relations: Vec<UmlRelation>,
    pub candidates: Vec<UmlCandidate>,
    pub warnings: Vec<String>,
    pub truncated: bool,
    pub stats: UmlStats,
}

/// Whether one kind of diagram has something to show for a scope.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct UmlKindAvailability {
    pub kind: UmlKind,
    pub available: bool,
    /// What it would show, or why it is not available.
    pub reason: String,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct UmlAvailability {
    pub title: String,
    pub kinds: Vec<UmlKindAvailability>,
    /// The diagram most likely to help for this scope.
    pub suggested: Option<UmlKind>,
}

/// Where activity and sequence diagrams read function bodies from. The server reads them from
/// the project (enriched with semantic facts) one at a time, so only the flows a diagram
/// follows are loaded.
pub trait FlowSource: Sync {
    fn flow(&self, symbol: &str) -> Option<FlowGraph>;

    /// The text of a project file (declarations the IR does not keep, such as a property's
    /// initializer or a Rust `impl` header).
    fn source(&self, _file: &str) -> Option<String> {
        None
    }
}

/// No bodies (class, package and component diagrams do not need them).
pub struct NoFlows;

impl FlowSource for NoFlows {
    fn flow(&self, _symbol: &str) -> Option<FlowGraph> {
        None
    }
}

impl FlowSource for std::sync::Mutex<nodex_core::Project> {
    fn flow(&self, symbol: &str) -> Option<FlowGraph> {
        self.lock().ok()?.flow(symbol)
    }

    fn source(&self, file: &str) -> Option<String> {
        self.lock().ok()?.source(file)
    }
}

/// What a recorded run adds: how often each flow node and each function ran, and (for a
/// runtime sequence diagram) the recorded calls.
#[derive(Default, Clone)]
pub struct Runtime {
    /// Flow symbol → node id → times it ran.
    pub counts: std::collections::BTreeMap<String, std::collections::BTreeMap<String, u64>>,
    /// Flow symbol → times it was called.
    pub calls: std::collections::BTreeMap<String, u64>,
    /// Recorded calls in start order (each with its caller).
    pub spans: Vec<nodex_trace::CallSpan>,
}

/// What a diagram is built from.
pub struct UmlInput<'a> {
    pub graph: &'a StructureGraph,
    pub knowledge: &'a ProjectKnowledge,
    pub units: &'a [BuildUnit],
    /// Project name (for the root component).
    pub project: &'a str,
    pub flows: &'a dyn FlowSource,
    /// A recorded run, when there is one.
    pub runtime: Option<&'a Runtime>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UmlError {
    /// The scope names nothing in the project.
    NotFound(String),
    /// The kind is not built yet.
    Unsupported(UmlKind),
}

impl std::fmt::Display for UmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UmlError::NotFound(what) => write!(f, "{what} not found in the project"),
            UmlError::Unsupported(kind) => write!(f, "{kind:?} diagrams are not available yet"),
        }
    }
}

impl std::error::Error for UmlError {}

/// Builds one diagram.
pub fn build(input: &UmlInput, kind: UmlKind, scope: &UmlScope, opts: &UmlOptions) -> Result<UmlDiagram, UmlError> {
    let index = Index::new(input.graph, input.knowledge);
    let scoped = Scoped::resolve(&index, scope)?;
    let mut diagram = match kind {
        UmlKind::Class => class::build(&index, &scoped, opts),
        UmlKind::Package => package::build(&index, &scoped, opts),
        UmlKind::Component => component::build(&index, &scoped, input, opts),
        UmlKind::Activity => activity::build(&index, &scoped, scope, input, opts),
        UmlKind::Sequence if opts.runtime => sequence::recorded(&index, &scoped, input, opts),
        UmlKind::Sequence => sequence::build(&index, &scoped, scope, input, opts),
        UmlKind::State => state::build(&index, &scoped, input, opts),
        UmlKind::UseCase => usecase::build(&index, &scoped, input, opts),
    };
    if diagram.title.is_empty() {
        diagram.title = scoped.title.clone();
    }
    diagram.stats.elements = diagram.elements.len() as u32;
    diagram.stats.relations = diagram.relations.len() as u32;
    Ok(diagram)
}

pub(crate) fn empty(kind: UmlKind) -> UmlDiagram {
    UmlDiagram {
        kind,
        title: String::new(),
        mermaid: String::new(),
        elements: Vec::new(),
        relations: Vec::new(),
        candidates: Vec::new(),
        warnings: Vec::new(),
        truncated: false,
        stats: UmlStats::default(),
    }
}
