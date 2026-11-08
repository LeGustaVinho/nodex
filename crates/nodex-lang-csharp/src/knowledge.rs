//! Knowledge facts of a C# file: ASP.NET routes (controllers and Minimal APIs), dependency
//! injection, configuration reads, HTTP clients, EF Core entities and writes, tests, entry
//! points and global error handlers.
//!
//! The detection rules were ported from CodeGraph (`crates/core/src/extract/csharp.rs`). This
//! pass walks the same syntax tree as the structure and flow passes, after them, and attaches
//! every fact to the Nodex symbol that owns its code.

use std::collections::{BTreeMap, HashMap};

use nodex_ir::knowledge::{
    AuthFact, ConfigFact, DiFact, EntityFact, EntryKind, FileKnowledge, GlobalHandlerFact, HttpCallFact, MutationFact, RouteFact,
    StateMachineFact, StateTransition,
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

    /// A named argument as a string literal: `Roles = "Admin,Ops"`.
    fn named(&self, name: &str) -> Option<String> {
        self.args.iter().find_map(|a| {
            let (key, value) = a.split_once('=')?;
            (key.trim() == name).then(|| unquote(value.trim())).flatten()
        })
    }
}

/// Authorization declared by attributes or endpoint conventions.
#[derive(Clone, Default)]
struct AuthSpec {
    /// `[Authorize]` (or a policy/role) is present.
    required: bool,
    anonymous: bool,
    roles: Vec<String>,
    policies: Vec<String>,
}

impl AuthSpec {
    fn from_attrs(attrs: &[Attr]) -> Self {
        let mut spec = AuthSpec::default();
        for a in attrs {
            match a.name.as_str() {
                "AllowAnonymous" => spec.anonymous = true,
                "Authorize" => {
                    spec.required = true;
                    if let Some(roles) = a.named("Roles") {
                        spec.add_roles(&roles);
                    }
                    if let Some(policy) = a.named("Policy").or_else(|| a.args.iter().find(|x| !x.contains('=')).and_then(|x| unquote(x))) {
                        spec.add_policy(&policy);
                    }
                }
                n if AUTH_MARKERS.contains(&n) => spec.required = true,
                _ => {}
            }
        }
        spec
    }

    fn add_roles(&mut self, text: &str) {
        for r in text.split(',').map(str::trim).filter(|r| !r.is_empty()) {
            if !self.roles.iter().any(|x| x == r) {
                self.roles.push(r.to_owned());
            }
        }
    }

    fn add_policy(&mut self, p: &str) {
        if !p.is_empty() && !self.policies.iter().any(|x| x == p) {
            self.policies.push(p.to_owned());
        }
    }

    /// The action's own attributes refine the class's: `[AllowAnonymous]` wins; roles add up.
    fn refine(&self, action: &AuthSpec) -> AuthSpec {
        if action.anonymous {
            return AuthSpec { anonymous: true, ..AuthSpec::default() };
        }
        let mut out = self.clone();
        out.required |= action.required;
        for r in &action.roles {
            out.add_roles(r);
        }
        for p in &action.policies {
            out.add_policy(p);
        }
        out
    }

    fn is_set(&self) -> bool {
        self.required || self.anonymous
    }

    /// `.RequireAuthorization("p")`, `.RequireAuthorization(p => p.RequireRole("Admin"))`.
    fn from_call(call: &Call) -> Option<Self> {
        let mut spec = AuthSpec::default();
        match call.name.as_str() {
            "AllowAnonymous" => spec.anonymous = true,
            "RequireAuthorization" => {
                spec.required = true;
                for a in &call.args {
                    if let Some(p) = unquote(a) {
                        spec.add_policy(&p);
                    }
                    let mut rest = a.as_str();
                    while let Some(at) = rest.find("RequireRole(") {
                        rest = &rest[at + "RequireRole(".len()..];
                        let inner = rest.split(')').next().unwrap_or("");
                        for r in inner.split(',').filter_map(unquote) {
                            spec.add_roles(&r);
                        }
                    }
                }
            }
            _ => return None,
        }
        Some(spec)
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
    /// Authorization attributes of the class.
    auth_spec: AuthSpec,
    /// Base types as written, with generic arguments (`IConsumer<OrderPlaced>`).
    bases: Vec<String>,
}

struct Walker<'a, 't> {
    src: Src<'a>,
    owners: &'a Owners,
    symbols: &'a [Symbol],
    out: FileKnowledge,
    /// `var api = app.MapGroup("/api")`: group variable → full prefix.
    groups: HashMap<String, String>,
    /// `var api = app.MapGroup("/api").RequireAuthorization()`: group variable → authorization.
    group_auth: HashMap<String, AuthSpec>,
    /// `var add = new Command("add")`: variable → command name.
    commands: HashMap<String, String>,
    /// State machines being collected: (owner type, machine variable) → fact.
    machines: BTreeMap<(String, String), StateMachineFact>,
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
        group_auth: HashMap::new(),
        commands: HashMap::new(),
        machines: BTreeMap::new(),
        callbacks,
        _tree: std::marker::PhantomData,
    };
    w.collect_groups(root);
    w.walk(root, None);
    let machines = std::mem::take(&mut w.machines);
    w.out.state_machines.extend(machines.into_values().map(nest_names).filter(|m| !m.states.is_empty() || !m.triggers.is_empty()));
    w.out
}

/// Substates are recorded as `Parent.Child`: transitions and the initial state use the same names.
fn nest_names(mut m: StateMachineFact) -> StateMachineFact {
    let full: HashMap<String, String> =
        m.states.iter().filter_map(|s| s.rsplit_once('.').map(|(_, short)| (short.to_owned(), s.clone()))).collect();
    let name = |s: &mut String| {
        if let Some(f) = full.get(s.as_str()) {
            *s = f.clone();
        }
    };
    for t in &mut m.transitions {
        name(&mut t.from);
        name(&mut t.to);
    }
    if let Some(i) = &mut m.initial {
        name(i);
    }
    m
}

/// `State.Off` → `Off`, `"Jump"` → `Jump`.
fn member_name(text: &str) -> String {
    let t = unquote(text).unwrap_or_else(|| text.trim().to_owned());
    t.rsplit('.').next().unwrap_or(&t).trim().to_owned()
}

/// The machine variable a Stateless chain configures: `_machine.Configure(State.A).Permit(...)` → `_machine`.
fn machine_of(receiver: &str) -> Option<String> {
    let at = receiver.find(".Configure(")?;
    let name = receiver[..at].trim().trim_start_matches("this.");
    Some(name.to_owned())
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
            "constructor_declaration" => self.constructor(node),
            "property_declaration" => self.property(node),
            "object_creation_expression" => self.creation(node),
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
        // Primary constructor parameters are injected like a constructor's.
        if let (Some(id), Some(list)) = (&id, first_child_of_kind(node, "parameter_list")) {
            let ctor = self
                .symbols
                .iter()
                .find(|s| s.kind == SymbolKind::Constructor && s.parent.as_deref() == Some(id.as_str()) && s.flavors.iter().any(|f| f == "primary-constructor"))
                .map(|s| s.id.clone())
                .unwrap_or_else(|| id.clone());
            self.injected_parameters(&ctor, list);
        }
        let full_bases: Vec<String> = first_child_of_kind(node, "base_list").map(|b| named_children(b).into_iter().map(|n| src.norm(n)).collect()).unwrap_or_default();
        ClassCtx {
            controller,
            auth: attrs.iter().any(|a| AUTH_MARKERS.contains(&a.name.as_str())),
            route,
            exception_filter: bases.iter().any(|b| matches!(b.as_str(), "IExceptionFilter" | "IAsyncExceptionFilter" | "IExceptionHandler")),
            auth_spec: AuthSpec::from_attrs(&attrs),
            bases: full_bases,
        }
    }

    /// `public OrderService(IOrderRepository repo, ILogger<OrderService> log)`: each service
    /// parameter is an injection point (core keeps those the container provides).
    fn constructor(&mut self, node: Node<'t>) {
        let Some(id) = self.owners.declared_at(node.start_byte(), node.end_byte(), |k| k == SymbolKind::Constructor).map(str::to_owned) else { return };
        if let Some(list) = node.child_by_field_name("parameters").or_else(|| first_child_of_kind(node, "parameter_list")) {
            self.injected_parameters(&id, list);
        }
    }

    fn injected_parameters(&mut self, owner: &str, list: Node<'t>) {
        let src = self.src;
        for p in children_of_kind(list, "parameter") {
            let Some(ty) = p.child_by_field_name("type").map(|t| src.norm(t)) else { continue };
            let bare = crate::syntax::bare_type_name(&ty);
            // Services are named types: not `int`, `string`, arrays or tuples.
            let simple = bare.rsplit('.').next().unwrap_or(&bare);
            if !simple.chars().next().is_some_and(char::is_uppercase) || matches!(simple, "String" | "Int32" | "Boolean" | "CancellationToken") || ty.starts_with('(') {
                continue;
            }
            self.out.di.push(DiFact {
                owner: owner.to_owned(),
                service: bare,
                implementation: None,
                lifetime: "inject".into(),
                token: None,
                framework: "aspnet".into(),
                span: src.span(p),
            });
        }
    }

    /// `new Command("add", "Adds an item")`, `new StateMachine<State, Trigger>(State.Off)`.
    fn creation(&mut self, node: Node<'t>) {
        let src = self.src;
        let Some(ty) = node.child_by_field_name("type").map(|t| src.norm(t)) else { return };
        let bare = crate::syntax::bare_type_name(&ty);
        let args: Vec<String> = node
            .child_by_field_name("arguments")
            .map(|l| children_of_kind(l, "argument").into_iter().map(|a| src.norm(a)).collect())
            .unwrap_or_default();
        let target = assigned_name(src, node);
        match bare.rsplit('.').next().unwrap_or(&bare) {
            "Command" | "RootCommand" => {
                if let Some(var) = target {
                    let name = if bare.ends_with("RootCommand") { "(root)".to_owned() } else { args.first().and_then(|a| unquote(a)).unwrap_or_else(|| var.clone()) };
                    self.commands.insert(var, name);
                }
            }
            "StateMachine" => {
                let (Some(var), Some(owner)) = (target, self.owners.type_at(node.start_byte()).map(str::to_owned)) else { return };
                let initial = args.first().map(|a| member_name(a));
                let span = src.span(node);
                let m = self.machine(&owner, &var, "stateless", span);
                if let Some(i) = initial {
                    if !m.states.contains(&i) {
                        m.states.push(i.clone());
                    }
                    m.initial = Some(i);
                }
            }
            _ => {}
        }
    }

    fn machine(&mut self, owner: &str, name: &str, framework: &str, span: nodex_ir::Span) -> &mut StateMachineFact {
        let name = name.trim_start_matches("this.").trim_start_matches('_').to_owned();
        self.machines.entry((owner.to_owned(), name.clone())).or_insert_with(|| StateMachineFact {
            owner: owner.to_owned(),
            name,
            framework: framework.into(),
            states: Vec::new(),
            initial: None,
            transitions: Vec::new(),
            triggers: Vec::new(),
            span,
        })
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
                let prefix = class.route.as_deref().unwrap_or("").replace("[action]", &name);
                let routes = action_routes(&attrs);
                let routed = !routes.is_empty();
                for (method, path) in routes {
                    self.route(&id, method, &join_aspnet(&prefix, &path.replace("[action]", &name)), "aspnet", node, None);
                }
                // `[Route]` on a class + no verb attribute on a public action: routed by convention.
                if !routed && class.route.is_some() && modifiers.iter().any(|m| m == "public") && attrs.iter().all(|a| a.name != "NonAction") {
                    let method = HTTP_ATTRS.iter().map(|(n, m)| (n.trim_start_matches("Http"), *m)).find(|(verb, _)| name.starts_with(verb)).map(|(_, m)| m);
                    if let Some(m) = method {
                        self.route(&id, m, &join_aspnet(&prefix, ""), "aspnet", node, None);
                    }
                }
                let anonymous = attrs.iter().any(|a| a.name == "AllowAnonymous");
                if !anonymous && (class.auth || attrs.iter().any(|a| AUTH_MARKERS.contains(&a.name.as_str()))) {
                    self.out.capability(&id, "AUTH_REQUIRED");
                }
                let spec = class.auth_spec.refine(&AuthSpec::from_attrs(&attrs));
                if spec.is_set() && (routed || class.route.is_some()) {
                    self.auth(&id, &spec, "aspnet", node);
                }
            }
            // Background work and message consumers the host calls.
            let base = |n: &str| class.bases.iter().find(|b| b.split('<').next().unwrap_or(b).rsplit('.').next() == Some(n));
            let generic_arg = |b: &String| b.split_once('<').map(|(_, a)| a.trim_end_matches('>').to_owned()).unwrap_or_default();
            match name.as_str() {
                "ExecuteAsync" if base("BackgroundService").is_some() => self.out.entry(&id, EntryKind::Job, "background service"),
                "StartAsync" if base("IHostedService").is_some() => self.out.entry(&id, EntryKind::Job, "hosted service"),
                "Execute" if base("IJob").is_some() => self.out.entry(&id, EntryKind::Job, "Quartz job"),
                "Invoke" if base("IInvocable").is_some() => self.out.entry(&id, EntryKind::Job, "Coravel job"),
                "Consume" => {
                    if let Some(b) = base("IConsumer") {
                        self.out.entry(&id, EntryKind::Message, format!("consumes {}", generic_arg(b)));
                    }
                }
                "Handle" => {
                    if let Some(b) = base("INotificationHandler").or_else(|| base("IHandleMessages")) {
                        self.out.entry(&id, EntryKind::Message, format!("handles {}", generic_arg(b)));
                    }
                }
                _ => {}
            }
            if class.exception_filter && matches!(name.as_str(), "OnException" | "OnExceptionAsync" | "TryHandleAsync") {
                self.out.handlers.push(GlobalHandlerFact { handler: id.clone(), exception: "*".into(), framework: "aspnet".into(), span: src.span(node) });
            }
        }
    }

    fn auth(&mut self, owner: &str, spec: &AuthSpec, framework: &str, node: Node) {
        self.out.auth.push(AuthFact {
            owner: owner.to_owned(),
            roles: spec.roles.clone(),
            policies: spec.policies.clone(),
            anonymous: spec.anonymous,
            framework: framework.into(),
            span: self.src.span(node),
        });
    }

    fn route(&mut self, handler: &str, method: &str, path: &str, framework: &str, node: Node, handler_ref: Option<String>) {
        self.out.routes.push(RouteFact {
            handler: handler.to_owned(),
            handler_ref,
            method: method.to_owned(),
            path: path.to_owned(),
            framework: framework.to_owned(),
            span: self.src.span(node),
            router: None,
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
        {
            // `app.MapGroup("/api").RequireAuthorization().WithTags("x")`: the group call is inside.
            let mut auth = AuthSpec::default();
            let mut group = None;
            for call in chain(self.src, init) {
                if let Some(a) = AuthSpec::from_call(&call) {
                    auth = auth.refine(&a);
                } else if call.name == "MapGroup" {
                    group = Some(call);
                    break;
                }
            }
            if let Some(call) = group
                && let Some(prefix) = call.string_arg(0)
            {
                let parent = self.groups.get(&call.receiver).cloned().unwrap_or_default();
                let full = join_group(&parent, &prefix);
                let var = self.src.norm(name);
                let inherited = self.group_auth.get(&call.receiver).cloned().unwrap_or_default();
                let merged = inherited.refine(&auth);
                if merged.is_set() {
                    self.group_auth.insert(var.clone(), merged);
                }
                self.groups.insert(var, full);
            }
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
                    token: None,
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
            let last = call.arg_nodes.last().copied();
            // A method group (`Handlers.Get`) may live in another file: `nodex-core` resolves the name.
            let handler_ref = last.filter(|h| matches!(h.kind(), "identifier" | "member_access_expression")).map(|h| src.norm(h));
            let handler = last
                .and_then(|h| match h.kind() {
                    "lambda_expression" | "anonymous_method_expression" => self.callbacks.get(&h.start_byte()).cloned(),
                    "identifier" | "member_access_expression" => {
                        let name = src.norm(h);
                        let name = name.rsplit('.').next().unwrap_or(&name).to_owned();
                        self.symbols.iter().find(|s| s.name == name && matches!(s.kind, SymbolKind::Method | SymbolKind::Function)).map(|s| s.id.clone())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| owner.clone());
            // Authorization of the group and of the calls chained on this one.
            let mut spec = self.group_auth.get(&call.receiver).cloned().unwrap_or_default();
            let mut cur = node.parent();
            while let Some(parent) = cur.filter(|p| p.kind() == "member_access_expression").and_then(|m| m.parent()).filter(|i| i.kind() == "invocation_expression") {
                if let Some(c) = Call::read(src, parent)
                    && let Some(a) = AuthSpec::from_call(&c)
                {
                    spec = spec.refine(&a);
                }
                cur = parent.parent();
            }
            // A handler without a Blueprint of its own is the registering function: not the endpoint.
            if spec.is_set() && handler != owner {
                self.auth(&handler, &spec, "aspnet-minimal", node);
                if !spec.anonymous {
                    self.out.capability(&handler, "AUTH_REQUIRED");
                }
            }
            self.route(&handler, verb, &join_group(&prefix, &path), "aspnet-minimal", node, handler_ref);
            return;
        }

        // System.CommandLine: `add.SetHandler(handler, option)`, `root.SetAction(...)`.
        if matches!(n, "SetHandler" | "SetAction")
            && let Some(command) = self.commands.get(call.receiver.trim_start_matches("this.")).cloned()
            && let Some(h) = call.arg_nodes.first().and_then(|a| self.callable(*a, &owner))
        {
            let label = if command == "(root)" { "root command".to_owned() } else { format!("command {command}") };
            self.out.entry(&h, EntryKind::Command, label);
            return;
        }

        // Hangfire: `RecurringJob.AddOrUpdate("id", () => svc.Run(), Cron.Daily)`, `BackgroundJob.Enqueue(...)`.
        if matches!(call.receiver.as_str(), "RecurringJob" | "BackgroundJob" | "_recurringJobs" | "recurringJobManager")
            && matches!(n, "AddOrUpdate" | "Enqueue" | "Schedule")
        {
            let job = call.arg_nodes.iter().find(|a| a.kind() == "lambda_expression").and_then(|l| self.lambda_target(*l, call.generics.first()));
            if let Some(h) = job {
                let label = match (n, call.string_arg(0)) {
                    ("AddOrUpdate", Some(id)) => format!("recurring job {id}"),
                    ("AddOrUpdate", None) => "recurring job".to_owned(),
                    _ => "background job".to_owned(),
                };
                self.out.entry(&h, EntryKind::Job, label);
            }
            return;
        }

        // Stateless: `_machine.Configure(State.Off).Permit(Trigger.Switch, State.On)`.
        if matches!(n, "Permit" | "PermitIf" | "PermitReentry" | "PermitReentryIf" | "SubstateOf" | "InitialTransition" | "PermitDynamic")
            && let Some(var) = machine_of(&call.receiver)
            && let Some(from) = call_arg_text(&call.receiver, "Configure(")
            && let Some(type_owner) = self.owners.type_at(node.start_byte()).map(str::to_owned)
        {
            let from = member_name(&from);
            let span = src.span(node);
            let args = call.args.clone();
            let m = self.machine(&type_owner, &var, "stateless", span);
            if !m.states.contains(&from) {
                m.states.push(from.clone());
            }
            let (to, trigger, guard) = match n {
                "Permit" | "PermitIf" => (args.get(1).map(|a| member_name(a)), args.first().map(|a| member_name(a)), args.get(2).cloned()),
                "PermitReentry" | "PermitReentryIf" => (Some(from.clone()), args.first().map(|a| member_name(a)), args.get(1).cloned()),
                "PermitDynamic" => (Some("?".to_owned()), args.first().map(|a| member_name(a)), None),
                "InitialTransition" => (args.first().map(|a| member_name(a)), Some("initial".to_owned()), None),
                // `SubstateOf(State.Parent)`: nesting, recorded as `Parent.Child` names below.
                _ => {
                    if let Some(parent) = args.first().map(|a| member_name(a)) {
                        let nested = format!("{parent}.{from}");
                        if !m.states.contains(&parent) {
                            m.states.push(parent);
                        }
                        if let Some(at) = m.states.iter().position(|s| *s == from) {
                            m.states[at] = nested;
                        }
                    }
                    return;
                }
            };
            if let Some(to) = to {
                if to != "?" && !m.states.contains(&to) {
                    m.states.push(to.clone());
                }
                let t = StateTransition { from, to, trigger, guard };
                if !m.transitions.contains(&t) {
                    m.transitions.push(t);
                }
            }
            return;
        }

        // Unity Animator: triggers and parameters it sets, states it plays.
        if recv_lower.contains("anim")
            && matches!(n, "SetTrigger" | "ResetTrigger" | "SetBool" | "SetInteger" | "SetFloat" | "Play" | "CrossFade" | "CrossFadeInFixedTime" | "PlayInFixedTime")
            && let Some(literal) = call.string_arg(0)
            && let Some(type_owner) = self.owners.type_at(node.start_byte()).map(str::to_owned)
        {
            let method = self.symbols.iter().find(|s| s.id == owner).map(|s| s.name.clone()).unwrap_or_default();
            let span = src.span(node);
            let var = call.receiver.clone();
            let m = self.machine(&type_owner, &var, "unity-animator", span);
            if n.starts_with("Set") || n == "ResetTrigger" {
                if !m.triggers.contains(&literal) {
                    m.triggers.push(literal);
                }
            } else {
                if !m.states.contains(&literal) {
                    m.states.push(literal.clone());
                }
                let t = StateTransition { from: "*".into(), to: literal, trigger: (!method.is_empty()).then_some(method), guard: None };
                if !m.transitions.contains(&t) {
                    m.transitions.push(t);
                }
            }
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

    /// The method a handler argument runs: a lambda with a Blueprint, or a method group.
    fn callable(&self, arg: Node<'t>, fallback: &str) -> Option<String> {
        let src = self.src;
        match arg.kind() {
            "lambda_expression" | "anonymous_method_expression" => {
                self.callbacks.get(&arg.start_byte()).cloned().or_else(|| self.lambda_target(arg, None)).or_else(|| Some(fallback.to_owned()))
            }
            "identifier" | "member_access_expression" => {
                let name = src.norm(arg);
                let name = name.rsplit('.').next().unwrap_or(&name).to_owned();
                self.symbols.iter().find(|s| s.name == name && matches!(s.kind, SymbolKind::Method | SymbolKind::Function)).map(|s| s.id.clone())
            }
            _ => None,
        }
    }

    /// The method an expression lambda calls: `x => x.Run()` with `AddOrUpdate<IReports>` → `Run`
    /// of a type named like the generic argument (or the only method of that name).
    fn lambda_target(&self, lambda: Node<'t>, generic: Option<&String>) -> Option<String> {
        let src = self.src;
        let body = lambda.child_by_field_name("body")?;
        let text = src.norm(body);
        let call = text.split('(').next()?;
        let name = call.rsplit('.').next()?.trim();
        let candidates: Vec<&Symbol> = self.symbols.iter().filter(|s| s.name == name && s.kind == SymbolKind::Method).collect();
        let want = generic.map(|g| crate::syntax::bare_type_name(g));
        let owner_name = |s: &Symbol| s.parent.as_deref().and_then(|p| self.symbols.iter().find(|x| x.id == p)).map(|p| p.name.clone()).unwrap_or_default();
        let pick = match &want {
            Some(w) => candidates.iter().find(|s| {
                let o = owner_name(s);
                o == *w || format!("I{o}") == *w
            }),
            None => None,
        };
        pick.or_else(|| (candidates.len() == 1).then(|| &candidates[0])).map(|s| s.id.clone())
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

/// Routes of a controller action, before the controller prefix: a verb attribute's own template
/// (`[HttpGet("{id}")]`), or the action's `[Route]` template with the verbs that have none
/// (`[HttpGet] [Route("{id}")]` is one `GET {id}`). A `[Route]` beside verbs that all carry their
/// own template matches every method, as in ASP.NET.
fn action_routes(attrs: &[Attr]) -> Vec<(&'static str, String)> {
    let mut verbs: Vec<(&'static str, Option<String>)> = Vec::new();
    for a in attrs {
        if let Some((_, m)) = HTTP_ATTRS.iter().find(|(n, _)| *n == a.name) {
            verbs.push((m, a.first_string()));
        }
    }
    let paths: Vec<String> = attrs.iter().filter(|a| a.name == "Route").filter_map(Attr::first_string).collect();
    let mut out: Vec<(&'static str, String)> = verbs.iter().filter_map(|(m, p)| p.clone().map(|p| (*m, p))).collect();
    let bare: Vec<&'static str> = verbs.iter().filter(|(_, p)| p.is_none()).map(|(m, _)| *m).collect();
    match (bare.is_empty(), paths.is_empty()) {
        (true, false) => out.extend(paths.iter().map(|p| ("ANY", p.clone()))),
        (false, true) => out.extend(bare.iter().map(|m| (*m, String::new()))),
        (false, false) => out.extend(bare.iter().flat_map(|m| paths.iter().map(move |p| (*m, p.clone())))),
        (true, true) => {}
    }
    out
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

/// The invocations of a fluent chain, outermost first: `a.B().C()` → `C`, `B`.
fn chain<'t>(src: Src, outer: Node<'t>) -> Vec<Call<'t>> {
    let mut out = Vec::new();
    let mut cur = Some(outer);
    while let Some(node) = cur.filter(|n| n.kind() == "invocation_expression") {
        let Some(call) = Call::read(src, node) else { break };
        out.push(call);
        cur = node
            .child_by_field_name("function")
            .filter(|f| f.kind() == "member_access_expression")
            .and_then(|f| f.child_by_field_name("expression"));
    }
    out
}

/// The text of the last `name(...)` argument in a receiver: `_m.Configure(State.A).Permit(..)` → `State.A`.
fn call_arg_text(receiver: &str, name: &str) -> Option<String> {
    let at = receiver.rfind(name)? + name.len();
    let rest = &receiver[at..];
    let mut depth = 0;
    for (i, ch) in rest.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return Some(rest[..i].trim().to_owned()),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// The variable or field an expression is assigned to: `var x = new T()`, `_x = new T()`.
fn assigned_name(src: Src, node: Node) -> Option<String> {
    let parent = node.parent()?;
    match parent.kind() {
        "variable_declarator" => parent.child_by_field_name("name").or_else(|| first_child_of_kind(parent, "identifier")).map(|n| src.norm(n)),
        "equals_value_clause" => parent.parent().and_then(|d| d.child_by_field_name("name").or_else(|| first_child_of_kind(d, "identifier"))).map(|n| src.norm(n)),
        "assignment_expression" => parent.child_by_field_name("left").map(|n| src.norm(n).trim_start_matches("this.").to_owned()),
        _ => None,
    }
}
