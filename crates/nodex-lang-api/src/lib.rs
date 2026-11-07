//! # Nodex language API
//!
//! The extension contract every language plugs into. Adding a language means implementing
//! [`LanguageFrontend`] (and, optionally, the semantic/trace/debug capabilities). The core,
//! the IR and the web frontend do not change.
//!
//! The [`kit`] module is the "lowering kit": shared helpers that build flow graphs, compute
//! spans and stable ids, so a frontend only writes the CST → concept mapping.

pub mod kit;

use std::path::{Path, PathBuf};

pub use nodex_ir as ir;
use nodex_ir::{FileIr, Language, PackageRef, Span, SymbolId};

/// A source file handed to a frontend.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Project-relative path with forward slashes.
    pub rel_path: String,
    pub text: String,
}

/// What a frontend detected about a project root.
#[derive(Debug, Clone)]
pub struct ProjectDescriptor {
    pub language: Language,
    /// Display name (`.csproj` name, `package.json` name, crate name).
    pub name: String,
    /// Manifest files found (`Sample.csproj`, `Packages/manifest.json`, ...).
    pub manifests: Vec<PathBuf>,
    pub packages: Vec<PackageRef>,
    /// Flavors of the project as a whole (for example `unity`).
    pub flavors: Vec<String>,
}

/// A language frontend. Implementations must be cheap to call concurrently from several threads.
pub trait LanguageFrontend: Send + Sync {
    fn id(&self) -> Language;

    /// File extensions handled by this frontend, without the dot.
    fn extensions(&self) -> &'static [&'static str];

    /// Detects whether `root` contains a project for this language.
    fn detect(&self, root: &Path) -> Option<ProjectDescriptor>;

    /// Lowers one file to structure symbols and flow graphs. Must never fail: unknown syntax
    /// becomes `Opaque` nodes and parse problems become diagnostics.
    fn lower_file(&self, file: &SourceFile) -> FileIr;

    /// Semantic enrichment (call targets, types, inheritance). Arrives in milestone M3.
    fn semantic(&self) -> Option<Box<dyn SemanticProvider>> {
        None
    }

    /// Build-time instrumentation for live traces. Arrives in milestone M4.
    fn instrumenter(&self) -> Option<Box<dyn TraceInstrumenter>> {
        None
    }

    /// Debug Adapter Protocol launch settings. Arrives in milestone M8.
    fn debugger(&self) -> Option<DapLaunchConfig> {
        None
    }
}

/// Resolves what syntax alone cannot: which symbol a call targets, the type of an expression,
/// inheritance across files and assemblies. Implemented by sidecars (Roslyn, TypeScript API,
/// rust-analyzer).
pub trait SemanticProvider: Send + Sync {
    /// Resolves the symbol referenced at `span` (a call site, a type name, a member access).
    fn resolve(&self, span: &Span) -> Option<SymbolId>;
    /// Returns the type of the expression at `span`, as display text.
    fn type_of(&self, span: &Span) -> Option<String>;
}

/// Prepares an instrumented build that streams trace events to the Nodex server.
pub trait TraceInstrumenter: Send + Sync {
    /// Produces the instrumented artifacts and returns the command that runs them.
    fn prepare(&self, root: &Path, out_dir: &Path) -> Result<RunCommand, String>;
}

/// A process the server starts to run an instrumented program.
#[derive(Debug, Clone)]
pub struct RunCommand {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

/// How to start a Debug Adapter Protocol adapter for a language.
#[derive(Debug, Clone)]
pub struct DapLaunchConfig {
    pub adapter: String,
    pub args: Vec<String>,
}
