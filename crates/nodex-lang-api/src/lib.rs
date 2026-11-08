//! # Nodex language API
//!
//! The extension contract every language plugs into. Adding a language means implementing
//! [`LanguageFrontend`] (and, optionally, the semantic/trace/debug capabilities). The core,
//! the IR and the web frontend do not change.
//!
//! The [`kit`] module is the "lowering kit": shared helpers that build flow graphs, compute
//! spans and stable ids, so a frontend only writes the CST → concept mapping.

pub mod kit;
pub mod knowledge;
pub mod ops;
pub mod sidecar;
pub mod syntax;

use std::path::{Path, PathBuf};

pub use nodex_ir as ir;
use nodex_ir::semantic::SemanticFacts;
use nodex_ir::{FileIr, Language, PackageRef};

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

    /// Starts a semantic provider for the project at `root` (call targets, types, inheritance).
    /// Returns `Err` with a human-readable reason when the provider cannot run (missing
    /// toolchain, sidecar not built). Frontends without semantics return `None`.
    fn semantic(&self, root: &Path) -> Option<Result<Box<dyn SemanticProvider>, String>> {
        let _ = root;
        None
    }

    /// Build-time instrumentation for live traces.
    fn instrumenter(&self) -> Option<Box<dyn TraceInstrumenter>> {
        None
    }

    /// Debug Adapter Protocol launch settings. Arrives in milestone M8.
    fn debugger(&self) -> Option<DapLaunchConfig> {
        None
    }
}

/// Resolves what syntax alone cannot: which symbol a call targets, the type of a local,
/// inheritance across files and assemblies. Implemented by sidecars (Roslyn, TypeScript API,
/// rust-analyzer). Calls may be slow; the server runs them on a background thread.
pub trait SemanticProvider: Send {
    /// One-line description (`Roslyn · MSBuild · Sample.csproj`).
    fn describe(&self) -> String;
    /// Analyzes the whole project.
    fn analyze(&mut self) -> Result<SemanticFacts, String>;
    /// Re-reads the changed files (absolute paths; deleted files included) and returns fresh
    /// facts for the whole project, so cross-file targets stay consistent.
    fn update(&mut self, changed: &[PathBuf]) -> Result<SemanticFacts, String>;
}

/// Prepares an instrumented build that streams trace events to the Nodex server.
pub trait TraceInstrumenter: Send + Sync {
    /// Builds and instruments the project at `root` into `out_dir` (never into the user's own
    /// build output) and returns the command that runs it. Progress lines go to `log`.
    /// Blocks for as long as the build takes.
    fn prepare(&self, root: &Path, out_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<RunCommand, String>;
}

/// A process the server starts to run an instrumented program. The server adds the
/// `NODEX_TRACE` and `NODEX_TRACE_TOKEN` environment variables.
#[derive(Debug, Clone)]
pub struct RunCommand {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// The instrumenter's map from (method, point) to source ranges.
    pub trace_map: PathBuf,
    pub description: String,
}

/// How to start a Debug Adapter Protocol adapter for a language.
#[derive(Debug, Clone)]
pub struct DapLaunchConfig {
    pub adapter: String,
    pub args: Vec<String>,
}
