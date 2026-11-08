//! Knowledge facts of a C# file: ASP.NET routes (controllers and Minimal APIs), dependency
//! injection, configuration reads, HTTP clients, EF Core entities and writes, tests, entry
//! points and global error handlers.
//!
//! The detection rules were ported from CodeGraph (`crates/core/src/extract/csharp.rs`). This
//! pass walks the same syntax tree as the structure and flow passes, after them, and attaches
//! every fact to the Nodex symbol that owns its code.

use std::collections::HashMap;

use nodex_ir::knowledge::{
    ConfigFact, DiFact, EntityFact, EntryKind, FileKnowledge, GlobalHandlerFact, HttpCallFact, MutationFact, RouteFact,
};
use nodex_ir::{Symbol, SymbolKind};
use nodex_lang_api::knowledge::{AUTH_MARKERS, Owners, call_capabilities, http_method_of, unquote};
use tree_sitter::Node;

use crate::syntax::{Src, children_of_kind, first_child_of_kind, named_children};

/// An attribute as written: `[HttpGet("{id}")]` → name `HttpGet`, args `["\"{id}\""]`.
struct Attr {
    name: String,
    args: Vec<String>,
}

impl Attr {
    /// First positional argument as a string literal.
    fn first_string(&self) -> Option<String> {
        self.args.iter().find(|a| !a.contains('=') || a.trim_start().starts_with('"')).and_then(|a| unquote(a))
    }
}

fn attributes(src: Src, decl: Node) -> Vec<Attr> {
    let mut out = Vec::new();
    for list in children_of_kind(decl, "attribute_list") {
        for attr in children_of_kind(list, "attribute") {
            let Some(name) = attr.child_by_field_name("name") else { continue };
            let name = src.norm(name);
            let name = name.rsplit('.').next().unwrap_or(&name).trim_end_matches("Attribute").to_owned();
            let args = first_child_of_kind(attr, "attribute_argument_list")
                .map(|l| children_of_kind(l, "attribute_argument").into_iter().map(|a| src.norm(a)).collect())
                .unwrap_or_default();
            out.push(Attr { name, args });
        }
    }
    out
}

fn base_names(src: Src, decl: Node) -> Vec<String> {
    first_child_of_kind(decl, "base_list")
        .map(|b| {
            named_children(b)
                .into_iter()
                .map(|n| {
                    let t = src.norm(n);
                    let t = t.split('<').next().unwrap_or(&t).to_owned();
                    t.rsplit('.').next().unwrap_or(&t).to_owned()
                })
                .collect()
        })
        .unwrap_or_default()
}

const HTTP_ATTRS: &[(&str, &str)] = &[
    ("HttpGet", "GET"),
    ("HttpPost", "POST"),
    ("HttpPut", "PUT"),
    ("HttpDelete", "DELETE"),
    ("HttpPatch", "PATCH"),
    ("HttpHead", "HEAD"),
    ("HttpOptions", "OPTIONS"),
];

const TEST_ATTRS: &[&str] = &[
    "Fact", "Theory", "Test", "TestMethod", "TestCase", "TestCaseSource", "DataTestMethod", "SkippableFact", "SkippableTheory", "UnityTest",
];
const TEST_HOOKS: &[&str] = &["SetUp", "TearDown", "OneTimeSetUp", "OneTimeTearDown", "TestInitialize", "TestCleanup", "ClassInitialize", "ClassCleanup"];

struct ClassCtx {
    controller: bool,
    /// `[Authorize]` on the class: every action requires an authenticated caller.
    auth: bool,
    route: Option<String>,
    exception_filter: bool,
}

struct Walker<'a, 't> {
    src: Src<'a>,
    owners: &'a Owners,
    symbols: &'a [Symbol],
    out: FileKnowledge,
    /// `var api = app.MapGroup("/api")`: group variable → full prefix.
    groups: HashMap<String, String>,
    /// Lambdas with a Blueprint of their own, by start byte.
    callbacks: &'a HashMap<usize, String>,
    _tree: std::marker::PhantomData<&'t ()>,
}

pub fn extract(src: Src, root: Node, symbols: &[Symbol], callbacks: &HashMap<usize, String>) -> FileKnowledge {
    let owners = Owners::new(symbols, src.file);
    let mut w = Walker {
        src,
        owners: &owners,
        symbols,
        out: FileKnowledge::default(),
        groups: HashMap::new(),
        callbacks,
        _tree: std::marker::PhantomData,
    };
    w.collect_groups(root);
    w.walk(root, None);
    w.out
}

impl<'a, 't> Walker<'a, 't> {
    fn walk(&mut self, node: Node<'t>, class: Option<&ClassCtx>) {
        match node.kind() {
            "class_declaration" | "record_declaration" | "struct_declaration" => {
                let ctx = self.class(node);
                for child in named_children(node) {
                    self.walk(child, Some(&ctx));
                }
                return;
            }
            "method_declaration" => self.method(node, class),
            "property_declaration" => self.property(node),
            "invocation_expression" => self.invocation(node),
            "element_access_expression" => self.element_access(node),
            _ => {}
        }
        for child in named_children(node) {
            self.walk(child, class);
        }
    }

    fn type_id(&self, node: Node) -> Option<String> {
        self.owners.declared_at(node.start_byte(), node.end_byte(), SymbolKind::is_type).map(str::to_owned)
    }

    fn class(&mut self, node: Node<'t>) -> ClassCtx {
        let src = self.src;
        let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
        let attrs = attributes(src, node);
        let bases = base_names(src, node);
        let has = |n: &str| attrs.iter().any(|a| a.name == n);
        let controller =
            name.ends_with("Controller") || has("ApiController") || has("Controller") || bases.iter().any(|b| b == "Controller" || b == "ControllerBase");
        let route = attrs.iter().find(|a| a.name == "Route").and_then(Attr::first_string).map(|r| {
            r.replace("[controller]", name.trim_end_matches("Controller"))
        });
        let id = self.type_id(node);
        if let Some(id) = &id {
            if has("TestClass") || has("TestFixture") {
                self.out.symbol_mut(id).test = true;
            }
            if let Some(table) = attrs.iter().find(|a| a.name == "Table").and_then(Attr::first_string) {
                self.out.entities.push(EntityFact {
                    owner: id.clone(),
                    entity: name.clone(),
                    property: None,
                    framework: "efcore".into(),
                    table: Some(table),
                    span: src.span(node),
                });
            }
        }
        ClassCtx {
            controller,
            auth: attrs.iter().any(|a| AUTH_MARKERS.contains(&a.name.as_str())),
            route,
            exception_filter: bases.iter().any(|b| matches!(b.as_str(), "IExceptionFilter" | "IAsyncExceptionFilter" | "IExceptionHandler")),
        }
    }

    fn method(&mut self, node: Node<'t>, class: Option<&ClassCtx>) {
        let src = self.src;
        let Some(id) = self
            .owners
            .declared_at(node.start_byte(), node.end_byte(), |k| matches!(k, SymbolKind::Method | SymbolKind::Function))
            .map(str::to_owned)
        else {
            return;
        };
        let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
        let attrs = attributes(src, node);
        let modifiers: Vec<String> = children_of_kind(node, "modifier").into_iter().map(|m| src.norm(m)).collect();

        if name == "Main" && modifiers.iter().any(|m| m == "static") {
            self.out.entry(&id, EntryKind::Main, "program entry (Main)");
        }
        for a in &attrs {
            if TEST_ATTRS.contains(&a.name.as_str()) {
                self.out.symbol_mut(&id).test = true;
                self.out.entry(&id, EntryKind::Test, format!("test [{}]", a.name));
            } else if TEST_HOOKS.contains(&a.name.as_str()) {
                self.out.entry(&id, EntryKind::Test, format!("test hook [{}]", a.name));
            } else if a.name == "Function" || a.name == "FunctionName" {
                self.out.entry(&id, EntryKind::Export, "serverless function");
            }
        }
        if let Some(class) = class {
            if class.controller {
                let prefix = class.route.as_deref().unwrap_or("");
                let mut routed = false;
                for a in &attrs {
                    let method = HTTP_ATTRS.iter().find(|(n, _)| *n == a.name).map(|(_, m)| *m);
                    let method = match (method, a.name.as_str()) {
                        (Some(m), _) => m,
                        (None, "Route") => "ANY",
                        _ => continue,
                    };
                    let path = a.first_string().unwrap_or_default().replace("[action]", &name);
                    self.route(&id, method, &join_aspnet(prefix, &path), "aspnet", node);
                    routed = true;
                }
                // `[Route]` on a class + no verb attribute on a public action: routed by convention.
                if !routed && class.route.is_some() && modifiers.iter().any(|m| m == "public") && attrs.iter().all(|a| a.name != "NonAction") {
                    let method = HTTP_ATTRS.iter().map(|(n, m)| (n.trim_start_matches("Http"), *m)).find(|(verb, _)| name.starts_with(verb)).map(|(_, m)| m);
                    if let Some(m) = method {
                        self.route(&id, m, &join_aspnet(prefix, ""), "aspnet", node);
                    }
                }
                let anonymous = attrs.iter().any(|a| a.name == "AllowAnonymous");
                if !anonymous && (class.auth || attrs.iter().any(|a| AUTH_MARKERS.contains(&a.name.as_str()))) {
                    self.out.capability(&id, "AUTH_REQUIRED");
                }
            }
            if class.exception_filter && matches!(name.as_str(), "OnException" | "OnExceptionAsync" | "TryHandleAsync") {
                self.out.handlers.push(GlobalHandlerFact { handler: id.clone(), exception: "*".into(), framework: "aspnet".into(), span: src.span(node) });
            }
        }
    }

    fn route(&mut self, handler: &str, method: &str, path: &str, framework: &str, node: Node) {
        self.out.routes.push(RouteFact {
            handler: handler.to_owned(),
            method: method.to_owned(),
            path: path.to_owned(),
            framework: framework.to_owned(),
            span: self.src.span(node),
        });
    }

    /// `DbSet<Order> Orders { get; set; }` in a context names the `Order` entity.
    fn property(&mut self, node: Node<'t>) {
        let src = self.src;
        let Some(ty) = node.child_by_field_name("type") else { return };
        let ty = src.norm(ty);
        let Some(entity) = ty.strip_prefix("DbSet<").and_then(|t| t.strip_suffix('>')) else { return };
        let Some(owner) = self.owners.type_at(node.start_byte()).map(str::to_owned) else { return };
        let property = node.child_by_field_name("name").map(|n| src.norm(n));
        self.out.entities.push(EntityFact {
            owner,
            entity: entity.trim().to_owned(),
            property,
            framework: "efcore".into(),
            table: None,
            span: src.span(node),
        });
    }

    /// Group variables of Minimal APIs: `var api = app.MapGroup("/api")`.
    fn collect_groups(&mut self, node: Node<'t>) {
        if node.kind() == "variable_declarator"
            && let Some(name) = node.child_by_field_name("name").or_else(|| first_child_of_kind(node, "identifier"))
            && let Some(init) = named_children(node).into_iter().rfind(|c| c.kind() == "invocation_expression")
            && let Some(call) = Call::read(self.src, init)
            && call.name == "MapGroup"
            && let Some(prefix) = call.string_arg(0)
        {
            let parent = self.groups.get(&call.receiver).cloned().unwrap_or_default();
            let full = join_group(&parent, &prefix);
            self.groups.insert(self.src.norm(name), full);
        }
        for child in named_children(node) {
            self.collect_groups(child);
        }
    }

    fn invocation(&mut self, node: Node<'t>) {
        let src = self.src;
        let Some(call) = Call::read(src, node) else { return };
        let Some(owner) = self.owners.function_at(node.start_byte()).map(str::to_owned) else { return };
        let recv_lower = call.receiver.to_ascii_lowercase();
        let n = call.name.as_str();
        for tag in call_capabilities(n, &call.receiver) {
            self.out.capability(&owner, tag);
        }

        // Dependency injection.
        let lifetime = match n {
            _ if n.starts_with("AddScoped") || n.starts_with("TryAddScoped") || n.starts_with("AddKeyedScoped") => Some("scoped"),
            _ if n.starts_with("AddSingleton") || n.starts_with("TryAddSingleton") || n.starts_with("AddKeyedSingleton") => Some("singleton"),
            _ if n.starts_with("AddTransient") || n.starts_with("TryAddTransient") || n.starts_with("AddKeyedTransient") => Some("transient"),
            "AddHostedService" => Some("hosted"),
            "AddDbContext" | "AddDbContextPool" | "AddDbContextFactory" => Some("context"),
            "AddHttpClient" => Some("client"),
            _ => None,
        };
        if let Some(lifetime) = lifetime {
            let (service, implementation) = match call.generics.as_slice() {
                [s, i, ..] => (Some(s.clone()), Some(i.clone())),
                [s] => (Some(s.clone()), None),
                [] => {
                    let types: Vec<String> = call.args.iter().filter_map(|a| a.strip_prefix("typeof(").and_then(|t| t.strip_suffix(')')).map(str::to_owned)).collect();
                    (types.first().cloned(), types.get(1).cloned())
                }
            };
            if let Some(service) = service {
                self.out.di.push(DiFact {
                    owner: owner.clone(),
                    service,
                    implementation,
                    lifetime: lifetime.into(),
                    framework: "aspnet".into(),
                    span: src.span(node),
                });
            }
            return;
        }

        // Minimal API routes.
        let verb = match n {
            "MapGet" => Some("GET"),
            "MapPost" => Some("POST"),
            "MapPut" => Some("PUT"),
            "MapDelete" => Some("DELETE"),
            "MapPatch" => Some("PATCH"),
            "Map" | "MapMethods" => Some("ANY"),
            _ => None,
        };
        if let Some(verb) = verb
            && let Some(path) = call.string_arg(0)
        {
            let prefix = self.groups.get(&call.receiver).cloned().unwrap_or_default();
            let handler = call
                .arg_nodes
                .last()
                .and_then(|h| match h.kind() {
                    "lambda_expression" | "anonymous_method_expression" => self.callbacks.get(&h.start_byte()).cloned(),
                    "identifier" | "member_access_expression" => {
                        let name = src.norm(*h);
                        let name = name.rsplit('.').next().unwrap_or(&name).to_owned();
                        self.symbols.iter().find(|s| s.name == name && matches!(s.kind, SymbolKind::Method | SymbolKind::Function)).map(|s| s.id.clone())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| owner.clone());
            self.route(&handler, verb, &join_group(&prefix, &path), "aspnet-minimal", node);
            return;
        }

        match n {
            "GetConnectionString" => self.config(&owner, call.string_arg(0).map(|k| format!("ConnectionStrings:{k}")), "connection-string", None, node),
            "GetValue" | "GetSection" | "GetRequiredSection" if recv_lower.contains("config") => {
                self.config(&owner, call.string_arg(0), "configuration", call.args.get(1).cloned(), node)
            }
            "GetEnvironmentVariable" => self.config(&owner, call.string_arg(0), "env", None, node),
            "GetInt" | "GetFloat" | "GetString" if recv_lower == "playerprefs" => self.config(&owner, call.string_arg(0), "player-prefs", call.args.get(1).cloned(), node),
            "UseExceptionHandler" | "UseDeveloperExceptionPage" | "UseStatusCodePages" => {
                self.out.handlers.push(GlobalHandlerFact { handler: owner.clone(), exception: "*".into(), framework: "aspnet".into(), span: src.span(node) });
            }
            "Run" | "RunAsync" | "Start" | "StartAsync"
                if ["app", "host", "webhost", "application"].iter().any(|w| recv_lower.trim_start_matches('_') == *w) =>
            {
                self.out.entry(&owner, EntryKind::ServerStart, "server start");
            }
            "SaveChanges" | "SaveChangesAsync" => self.mutation(&owner, &call.receiver, "database", node),
            "Add" | "AddAsync" | "AddRange" | "Update" | "UpdateRange" | "Remove" | "RemoveRange" | "Attach" | "ExecuteDelete"
            | "ExecuteDeleteAsync" | "ExecuteUpdate" | "ExecuteUpdateAsync"
                if is_db_receiver(&recv_lower) =>
            {
                self.mutation(&owner, &call.receiver, "database", node)
            }
            "Set" | "SetString" | "SetAsync" | "SetStringAsync" | "StringSet" | "StringSetAsync" | "Remove" | "RemoveAsync"
                if recv_lower.contains("cache") || recv_lower.contains("redis") =>
            {
                self.mutation(&owner, &call.receiver, "cache", node)
            }
            "SetInt" | "SetFloat" | "SetString" | "DeleteKey" | "DeleteAll" if recv_lower == "playerprefs" || recv_lower == "editorprefs" => {
                self.mutation(&owner, &call.receiver, "storage", node)
            }
            _ => {}
        }

        // HTTP clients.
        let http_name = matches!(
            n,
            "GetAsync" | "PostAsync" | "PutAsync" | "DeleteAsync" | "PatchAsync" | "SendAsync" | "GetStringAsync" | "GetStreamAsync"
                | "GetByteArrayAsync" | "GetFromJsonAsync" | "PostAsJsonAsync" | "PutAsJsonAsync" | "PatchAsJsonAsync" | "DeleteFromJsonAsync"
        );
        if http_name && (recv_lower.contains("http") || recv_lower.contains("client")) {
            if let Some(url) = call.args.first() {
                let method = http_method_of(n).unwrap_or("GET");
                self.http(&owner, method, url, "HttpClient", node);
            }
        } else if call.receiver == "UnityWebRequest" && matches!(n, "Get" | "Post" | "Put" | "Delete" | "Head" | "PostWwwForm") {
            if let Some(url) = call.args.first() {
                let method = http_method_of(n).unwrap_or("GET");
                self.http(&owner, method, url, "UnityWebRequest", node);
            }
        }
    }

    /// `config["Jwt:Key"]`, `Configuration["X"]`.
    fn element_access(&mut self, node: Node<'t>) {
        let src = self.src;
        let Some(target) = node.child_by_field_name("expression") else { return };
        let recv = src.norm(target).to_ascii_lowercase();
        if !(recv.ends_with("configuration") || recv.ends_with("config") || recv.ends_with("_config")) {
            return;
        }
        let Some(sub) = node.child_by_field_name("subscript").or_else(|| first_child_of_kind(node, "bracketed_argument_list")) else { return };
        let Some(arg) = named_children(sub).into_iter().next() else { return };
        let Some(owner) = self.owners.function_at(node.start_byte()).map(str::to_owned) else { return };
        self.config(&owner, unquote(&src.norm(arg)), "configuration", None, node);
    }

    fn config(&mut self, owner: &str, key: Option<String>, source: &str, default: Option<String>, node: Node) {
        let Some(key) = key else { return };
        self.out.config.push(ConfigFact { owner: owner.to_owned(), key, source: source.into(), default, span: self.src.span(node) });
    }

    fn mutation(&mut self, owner: &str, target: &str, kind: &str, node: Node) {
        self.out.mutations.push(MutationFact { owner: owner.to_owned(), target: target.to_owned(), kind: kind.into(), span: self.src.span(node) });
    }

    fn http(&mut self, owner: &str, method: &str, url: &str, client: &str, node: Node) {
        let url = unquote(url).unwrap_or_else(|| url.to_owned());
        self.out.http_calls.push(HttpCallFact {
            owner: owner.to_owned(),
            method: method.into(),
            url,
            client: client.into(),
            span: self.src.span(node),
        });
    }
}

/// `_db.Orders`, `context.Users`, `dbContext.Set<T>()`: a receiver that is a database context.
fn is_db_receiver(recv: &str) -> bool {
    let head = recv.trim_start_matches("this.").split('.').next().unwrap_or("").trim_start_matches('_');
    matches!(head, "db" | "context" | "ctx" | "dbcontext" | "database") || head.ends_with("context") || head.ends_with("db")
}

/// One invocation as written: `receiver.Name<Generics>(args)`.
struct Call<'t> {
    receiver: String,
    name: String,
    generics: Vec<String>,
    args: Vec<String>,
    arg_nodes: Vec<Node<'t>>,
}

impl<'t> Call<'t> {
    fn read(src: Src, node: Node<'t>) -> Option<Self> {
        let function = node.child_by_field_name("function")?;
        let (receiver, name_node) = match function.kind() {
            "member_access_expression" => (function.child_by_field_name("expression").map(|e| src.norm(e)).unwrap_or_default(), function.child_by_field_name("name")?),
            "conditional_access_expression" => return None,
            _ => (String::new(), function),
        };
        let (name, generics) = match name_node.kind() {
            "generic_name" => {
                let name = first_child_of_kind(name_node, "identifier").map(|i| src.norm(i)).unwrap_or_default();
                let generics = first_child_of_kind(name_node, "type_argument_list")
                    .map(|l| named_children(l).into_iter().map(|t| src.norm(t)).collect())
                    .unwrap_or_default();
                (name, generics)
            }
            _ => (src.norm(name_node), Vec::new()),
        };
        let arg_nodes: Vec<Node<'t>> = node
            .child_by_field_name("arguments")
            .map(|l| {
                children_of_kind(l, "argument")
                    .into_iter()
                    .filter_map(|a| named_children(a).into_iter().last())
                    .collect()
            })
            .unwrap_or_default();
        let args = arg_nodes.iter().map(|a| src.norm(*a)).collect();
        Some(Self { receiver, name, generics, args, arg_nodes })
    }

    fn string_arg(&self, i: usize) -> Option<String> {
        self.args.get(i).and_then(|a| unquote(a))
    }
}

/// Minimal API group join: `MapGroup("/api")` + `MapGet("/orders")` is `/api/orders`.
fn join_group(prefix: &str, path: &str) -> String {
    let joined = format!("{}/{}", prefix.trim_end_matches('/'), path.trim().trim_start_matches('/'));
    let joined = joined.trim_end_matches('/');
    if joined.starts_with('/') { joined.to_owned() } else { format!("/{joined}") }
}

/// ASP.NET route join: a leading `/` or `~/` on the action overrides the prefix.
pub fn join_aspnet(prefix: &str, path: &str) -> String {
    let s = path.trim();
    if s.starts_with('/') || s.starts_with('~') {
        return format!("/{}", s.trim_start_matches('~').trim_matches('/'));
    }
    let p = prefix.trim().trim_matches('/');
    let s = s.trim_matches('/');
    match (p.is_empty(), s.is_empty()) {
        (true, true) => "/".to_owned(),
        (true, false) => format!("/{s}"),
        (false, true) => format!("/{p}"),
        (false, false) => format!("/{p}/{s}"),
    }
}
