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

    /// Step-through debugging through a Debug Adapter Protocol adapter (milestone M8).
    fn debugger(&self) -> Option<Box<dyn DebugLauncher>> {
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
    fn prepare(&self, root: &Path, out_dir: &Path, env: &RunEnv, log: &mut dyn FnMut(&str)) -> Result<RunCommand, String>;

    /// Instruments one changed file again into a running copy (browser apps under a dev server
    /// that reloads it). `first_id` is the next free method id. Returns the new methods as the
    /// JSON array of the trace map's `methods`, or `None` when this run kind does not do it.
    fn reinstrument(&self, _root: &Path, _out_dir: &Path, _file: &Path, _first_id: i32) -> Option<Result<String, String>> {
        None
    }
}

/// Where a run's trace events go, known before the program is prepared (browser apps bake it
/// into the page).
#[derive(Debug, Clone)]
pub struct RunEnv {
    /// `host:port` of the TCP trace listener (`NODEX_TRACE`).
    pub tcp: String,
    /// WebSocket URL for programs that cannot open TCP sockets (browser pages).
    pub ws_url: String,
    /// Run token (`NODEX_TRACE_TOKEN`; the `run` query parameter of `ws_url`).
    pub token: String,
}

/// What kind of process a run starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RunKind {
    /// A program that runs and exits.
    #[default]
    Program,
    /// A dev server that serves an app until stopped; its URL is read from its output.
    DevServer,
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
    pub kind: RunKind,
    /// Extra environment variables for the process.
    pub env: Vec<(String, String)>,
}

/// How a debug adapter is fetched and started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterTransport {
    /// DAP over the adapter's stdin/stdout.
    Stdio,
    /// The adapter listens on a TCP port; `{port}` in [`AdapterSpec::args`] is replaced by a free one.
    Tcp,
}

/// One downloadable build of an adapter.
#[derive(Debug, Clone, Copy)]
pub struct AdapterDownload {
    /// `std::env::consts::OS` and `ARCH` it runs on (`any` for scripts).
    pub os: &'static str,
    pub arch: &'static str,
    pub url: &'static str,
    /// Lowercase hex SHA-256 of the archive.
    pub sha256: &'static str,
    pub size: u64,
    /// `zip` (also `.vsix`) or `tar.gz`.
    pub format: &'static str,
    /// The adapter's executable (or script) inside the unpacked archive.
    pub program: &'static str,
}

/// A debug adapter Nodex downloads on demand (after the user agrees) and starts.
#[derive(Debug, Clone, Copy)]
pub struct AdapterSpec {
    /// Folder-safe name: `netcoredbg`, `js-debug`, `codelldb`.
    pub name: &'static str,
    pub version: &'static str,
    /// The DAP `adapterID`.
    pub adapter_id: &'static str,
    /// Where it comes from (shown before downloading).
    pub homepage: &'static str,
    pub downloads: &'static [AdapterDownload],
    /// Environment variable that points to an adapter installed elsewhere (the program).
    pub env_override: &'static str,
    /// Run the program with this interpreter (`node` for js-debug).
    pub interpreter: Option<&'static str>,
    pub args: &'static [&'static str],
    pub transport: AdapterTransport,
}

impl AdapterSpec {
    /// The build for this machine.
    pub fn download(&self) -> Option<&'static AdapterDownload> {
        self.downloads
            .iter()
            .find(|d| (d.os == "any" || d.os == std::env::consts::OS) && (d.arch == "any" || d.arch == std::env::consts::ARCH))
    }
}

/// A program built for debugging and how the adapter should start it.
#[derive(Debug, Clone)]
pub struct DebugLaunch {
    /// Arguments of the DAP `launch` request.
    pub launch: serde_json::Value,
    pub description: String,
}

/// Debugging for one language: which adapter, and a debug build of the project.
pub trait DebugLauncher: Send + Sync {
    fn adapter(&self) -> AdapterSpec;
    /// Builds the project at `root` for debugging into `out_dir` and returns the `launch`
    /// arguments. Progress lines go to `log`. Blocks for as long as the build takes.
    fn prepare(&self, root: &Path, out_dir: &Path, log: &mut dyn FnMut(&str)) -> Result<DebugLaunch, String>;
}
