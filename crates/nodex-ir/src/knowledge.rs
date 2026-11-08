//! Knowledge facts: what an application *is*, beyond how its functions work.
//!
//! Frontends record, per file, the facts their syntax shows: HTTP routes and the code that calls
//! them, dependency-injection registrations, configuration reads, state mutations, tests, entry
//! points and data models. Every fact is owned by a Nodex symbol id and carries the span it was
//! read from. `nodex-core` links them project-wide into knowledge edges (`HttpCalls`, `ResolvesDi`,
//! `Tests`, `CaughtBy`, ...) and a [`ProjectKnowledge`] summary.
//!
//! The rules were ported from CodeGraph (see ADR-015). Unlike CodeGraph, structural relations
//! (calls, inheritance) come from compilers, so [`Confidence`] is only used for the knowledge
//! relations no compiler can prove, such as a front-end URL matching a back-end route.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Span, SymbolId};

/// How sure Nodex is of a knowledge relation that no compiler proves.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[ts(export)]
pub enum Confidence {
    /// Proven by the code itself: an explicit registration, a literal key, a direct call.
    Exact,
    /// Deduced from a convention or a unique match (one route matches the URL).
    Inferred,
    /// The code shows the relation but not its target (several candidates, a dynamic URL).
    Unresolved,
}

impl Confidence {
    /// The weaker of two confidences, for relations that chain several hops.
    pub fn weakest(self, other: Confidence) -> Confidence {
        self.max(other)
    }
}

/// An HTTP endpoint declared by the code (`[HttpGet("{id}")]`, `app.get('/x', h)`, `#[get("/x")]`).
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct RouteFact {
    /// The function that handles the request.
    pub handler: SymbolId,
    /// The handler as written when it is a name (`ordersController.list`, `Handlers.Get`,
    /// `handlers::list`). `nodex-core` resolves it over the whole project, so handlers declared in
    /// another file are found; `handler` is the frontend's best guess within the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub handler_ref: Option<String>,
    /// Upper-case HTTP method (`GET`), `ANY` when every method matches, or `PAGE` for front-end routes.
    pub method: String,
    /// Path as declared, joined with the prefixes known in the same file (controller `[Route]`,
    /// `MapGroup`, Nest `@Controller`). Project-wide prefixes come from [`RoutePrefix`].
    pub path: String,
    /// `aspnet`, `aspnet-minimal`, `express`, `fastify`, `nestjs`, `nextjs`, `axum`, `actix`, ...
    pub framework: String,
    pub span: Span,
}

/// A prefix applied to routes declared elsewhere: `app.use('/api', router)`,
/// `app.setGlobalPrefix('api')`, `register(plugin, { prefix })`.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct RoutePrefix {
    /// `global` (every route of the framework), `mount` (the routes of `target`).
    pub kind: String,
    /// Mounted router or plugin as written (`ordersRouter`, `./routes/orders`), empty for `global`.
    pub target: String,
    pub prefix: String,
    pub framework: String,
    pub span: Span,
}

/// An outgoing HTTP request (`fetch('/api/orders')`, `http.GetAsync(url)`, `reqwest::get(u)`).
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct HttpCallFact {
    /// The function that makes the request.
    pub owner: SymbolId,
    /// Upper-case HTTP method; `GET` when the call does not say.
    pub method: String,
    /// URL as written: a literal, a template (`` `/api/orders/${id}` ``) or `p:<index>` when it is
    /// the owner's parameter (the owner is then an HTTP wrapper).
    pub url: String,
    /// `fetch`, `axios`, `HttpClient`, `reqwest`, ...
    pub client: String,
    pub span: Span,
}

/// A dependency-injection registration (`services.AddScoped<IRepo, Repo>()`, Nest `providers`).
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct DiFact {
    /// The function that registers it (`Program.<Main>$`, a module class).
    pub owner: SymbolId,
    /// Service type or injection token as written.
    pub service: String,
    /// Implementation type as written, when it differs from the service.
    pub implementation: Option<String>,
    /// `scoped`, `singleton`, `transient`, `hosted`, `context`, `client`, `class`, `value`,
    /// `factory`, `existing`, or `inject` for an injection point (`@Inject("TOKEN") x: Service`).
    pub lifetime: String,
    /// Injection token that links a registration to its injection points (`provide: "TOKEN"`).
    #[serde(default)]
    pub token: Option<String>,
    pub framework: String,
    pub span: Span,
}

/// A configuration value read by the code (`config["Db:Url"]`, `process.env.API_URL`, `env::var("X")`).
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct ConfigFact {
    pub owner: SymbolId,
    pub key: String,
    /// `env`, `configuration`, `connection-string`, `player-prefs`, ...
    pub source: String,
    pub default: Option<String>,
    pub span: Span,
}

/// State the code changes outside its own locals.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct MutationFact {
    pub owner: SymbolId,
    /// What is changed, as written (`_db.Orders`, `Order`, `cache`).
    pub target: String,
    /// `database`, `cache`, `storage`, `global`.
    pub kind: String,
    pub span: Span,
}

/// A framework-level error handler (`app.UseExceptionHandler`, `IExceptionFilter`, Nest `@Catch`).
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct GlobalHandlerFact {
    pub handler: SymbolId,
    /// Exception type handled, `*` for every error.
    pub exception: String,
    pub framework: String,
    pub span: Span,
}

/// A persisted entity named by the code: an EF Core `DbSet<Order> Orders`, a `[Table("orders")]`
/// class, a TypeORM `@Entity()`, a diesel `table!`. `nodex-core` resolves the entity type and
/// reads its fields from the structure graph.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct EntityFact {
    /// The symbol that names it: the database context, or the entity type itself.
    pub owner: SymbolId,
    /// Entity type as written (`Order`).
    pub entity: String,
    /// Property of the context that exposes the entity (`Orders`), used to link mutations.
    pub property: Option<String>,
    /// `efcore`, `typeorm`, `diesel`, `sea-orm`, ...
    pub framework: String,
    pub table: Option<String>,
    pub span: Span,
}

/// Why a function runs without being called by project code.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[ts(export)]
pub enum EntryKind {
    /// Program entry (`Main`, top-level statements, Rust `main`).
    Main,
    /// Handles an HTTP route.
    Route,
    /// Starts a server (`app.Run()`, `app.listen()`).
    ServerStart,
    /// Serverless or exported function (`[Function]`, wasm/FFI export, Tauri command).
    Export,
    /// Message, event or schedule handler (`@EventPattern`, `@Cron`).
    EventHandler,
    /// Called by an engine or framework at a lifecycle moment (Unity messages, React components).
    Framework,
    /// Test case or test fixture hook.
    Test,
}

/// An entry point and the reason the code gives for it.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct EntryPoint {
    pub kind: EntryKind,
    /// Plain-language reason (`GET /api/orders/{id}`, `test hook [SetUp]`, `server start`).
    pub label: String,
}

/// A field of a [`DataModel`].
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct ModelField {
    pub name: String,
    pub ty: Option<String>,
    pub primary_key: bool,
    pub foreign_key: bool,
    /// Related model, as written, for navigation properties and foreign keys.
    pub target: Option<String>,
}

/// A persisted entity (EF Core, TypeORM, diesel, SeaORM).
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct DataModel {
    pub framework: String,
    /// Table name when the code sets one.
    pub table: Option<String>,
    pub fields: Vec<ModelField>,
}

/// Knowledge attached to one symbol.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct SymbolFacts {
    pub symbol: SymbolId,
    pub entry: Option<EntryPoint>,
    /// A test case (or a class of test cases).
    pub test: bool,
    pub data_model: Option<DataModel>,
    /// Side effects the syntax shows (`DB_READ`, `NETWORK_IO`, `FILE_IO`, `LOGGING`, `AUTH_REQUIRED`).
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// Every knowledge fact of one file.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Default)]
#[ts(export)]
pub struct FileKnowledge {
    pub routes: Vec<RouteFact>,
    pub prefixes: Vec<RoutePrefix>,
    pub http_calls: Vec<HttpCallFact>,
    pub di: Vec<DiFact>,
    pub config: Vec<ConfigFact>,
    pub mutations: Vec<MutationFact>,
    pub handlers: Vec<GlobalHandlerFact>,
    pub entities: Vec<EntityFact>,
    pub symbols: Vec<SymbolFacts>,
}

impl FileKnowledge {
    pub fn is_empty(&self) -> bool {
        *self == FileKnowledge::default()
    }

    /// The facts of `symbol`, created on first use.
    pub fn symbol_mut(&mut self, symbol: &str) -> &mut SymbolFacts {
        match self.symbols.iter().position(|s| s.symbol == symbol) {
            Some(i) => &mut self.symbols[i],
            None => {
                self.symbols.push(SymbolFacts {
                    symbol: symbol.to_owned(),
                    entry: None,
                    test: false,
                    data_model: None,
                    capabilities: Vec::new(),
                });
                self.symbols.last_mut().expect("just pushed")
            }
        }
    }

    /// Marks `symbol` as an entry point, keeping the first reason found.
    pub fn entry(&mut self, symbol: &str, kind: EntryKind, label: impl Into<String>) {
        let facts = self.symbol_mut(symbol);
        if facts.entry.is_none() {
            facts.entry = Some(EntryPoint { kind, label: label.into() });
        }
    }

    /// Adds a side-effect tag to `symbol`.
    pub fn capability(&mut self, symbol: &str, tag: &str) {
        let facts = self.symbol_mut(symbol);
        if !facts.capabilities.iter().any(|c| c == tag) {
            facts.capabilities.push(tag.to_owned());
        }
    }
}

/// A route after project-wide prefixes were applied.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct ResolvedRoute {
    pub handler: SymbolId,
    pub method: String,
    /// Full path as declared (`/api/orders/{id}`).
    pub path: String,
    /// Comparable form: lower case, parameters collapsed (`/api/orders/{}`).
    pub normalized: String,
    pub framework: String,
    pub span: Span,
}

/// Per-symbol metrics and tags computed over the whole project.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct SymbolInfo {
    pub symbol: SymbolId,
    /// Cyclomatic complexity of the body (1 = straight line).
    pub complexity: u32,
    /// Side effects: `DB_READ`, `DB_WRITE`, `NETWORK_IO`, `FILE_IO`, `CACHE`, `LOGGING`,
    /// `CONFIG_READ`, `THROWS`, `ASYNC`, `AUTH_REQUIRED`, ...
    pub capabilities: Vec<String>,
    pub entry: Option<EntryPoint>,
    pub test: bool,
}

/// A data model with the symbol that declares it.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct ModelInfo {
    pub symbol: SymbolId,
    pub model: DataModel,
}

/// Project-wide knowledge served next to the structure graph.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Default)]
#[ts(export)]
pub struct ProjectKnowledge {
    pub routes: Vec<ResolvedRoute>,
    pub http_calls: Vec<HttpCallFact>,
    pub di: Vec<DiFact>,
    pub config: Vec<ConfigFact>,
    pub models: Vec<ModelInfo>,
    /// Symbols with knowledge or a body, sorted by id.
    pub symbols: Vec<SymbolInfo>,
}
