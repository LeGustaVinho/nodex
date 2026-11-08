//! Knowledge facts of a TypeScript/JavaScript file: Express/Fastify, NestJS and Next.js routes,
//! NestJS dependency injection, HTTP clients (`fetch`, axios, Angular `HttpClient`), environment
//! and configuration reads, Prisma/TypeORM writes and entities, test cases, server start and
//! error handlers.
//!
//! The detection rules were ported from CodeGraph (`crates/core/src/extract/typescript.rs`). This
//! pass walks the same syntax tree as the structure and flow passes, after them.

use std::collections::HashMap;

use nodex_ir::knowledge::{
    AuthFact, ConfigFact, DiFact, EntityFact, EntryKind, FileKnowledge, GlobalHandlerFact, HttpCallFact, MutationFact, RouteFact, RoutePrefix,
    StateMachineFact, StateTransition,
};
use nodex_ir::{Symbol, SymbolKind};
use nodex_lang_api::knowledge::{AUTH_MARKERS, Owners, call_capabilities, unquote};
use tree_sitter::Node;

use crate::syntax::{Src, children_of_kind, named_children, parameters};

/// A decorator as written: `@Get(":id")` → name `Get`, the argument nodes.
struct Decorator<'t> {
    name: String,
    args: Vec<Node<'t>>,
}

fn decorator<'t>(src: Src, node: Node<'t>) -> Option<Decorator<'t>> {
    let expr = named_children(node).into_iter().next()?;
    match expr.kind() {
        "call_expression" => {
            let name = expr.child_by_field_name("function").map(|f| src.norm(f))?;
            let args = expr.child_by_field_name("arguments").map(named_children).unwrap_or_default();
            Some(Decorator { name: name.rsplit('.').next().unwrap_or(&name).to_owned(), args })
        }
        _ => Some(Decorator { name: src.norm(expr), args: Vec::new() }),
    }
}

/// Decorators of a class: its own, plus those written before `export` (`@Controller() export class`).
fn class_decorators<'t>(src: Src, class: Node<'t>) -> Vec<Decorator<'t>> {
    let mut nodes = children_of_kind(class, "decorator");
    if let Some(parent) = class.parent().filter(|p| p.kind() == "export_statement") {
        nodes.extend(children_of_kind(parent, "decorator"));
    }
    nodes.into_iter().filter_map(|d| decorator(src, d)).collect()
}

/// Decorators of a class member: the `decorator` siblings right before it.
fn member_decorators<'t>(src: Src, member: Node<'t>) -> Vec<Decorator<'t>> {
    let mut out = children_of_kind(member, "decorator").into_iter().filter_map(|d| decorator(src, d)).collect::<Vec<_>>();
    let mut cur = member.prev_named_sibling();
    while let Some(c) = cur {
        if c.kind() != "decorator" {
            break;
        }
        if let Some(d) = decorator(src, c) {
            out.push(d);
        }
        cur = c.prev_named_sibling();
    }
    out
}

fn string_of(src: Src, node: Node) -> Option<String> {
    match node.kind() {
        "string" | "template_string" => unquote(src.text(node)),
        _ => None,
    }
}

const NEST_VERBS: &[(&str, &str)] =
    &[("Get", "GET"), ("Post", "POST"), ("Put", "PUT"), ("Patch", "PATCH"), ("Delete", "DELETE"), ("Head", "HEAD"), ("Options", "OPTIONS"), ("All", "ANY")];
const ROUTE_VERBS: &[&str] = &["get", "post", "put", "patch", "delete", "del", "all", "options", "head"];
const CLIENT_VERBS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "request"];

struct Walker<'a> {
    src: Src<'a>,
    owners: &'a Owners,
    symbols: &'a [Symbol],
    function_ids: &'a HashMap<usize, String>,
    /// `express`, `fastify`, `koa`, `hono`: the server framework this file uses.
    server: &'static str,
    /// The file imports a server framework (receivers named `api`/`server` are routers only then).
    imports_server: bool,
    /// Local names bound to a project module: `import orders from "./routes/orders"` →
    /// `orders` → `src/routes/orders` (project path without extension).
    imports: HashMap<String, String>,
    /// Authorization applied to every route of a router: `router.use(requireAuth)`.
    router_auth: HashMap<String, Middleware>,
    /// The file uses React Router (`<Route path element>` and route objects are pages).
    react_router: bool,
    out: FileKnowledge,
}

/// Authorization read from route middlewares or decorators.
#[derive(Clone, Default)]
struct Middleware {
    roles: Vec<String>,
    policies: Vec<String>,
    anonymous: bool,
}

impl Middleware {
    fn is_empty(&self) -> bool {
        self.roles.is_empty() && self.policies.is_empty() && !self.anonymous
    }

    fn merge(&mut self, other: &Middleware) {
        for r in &other.roles {
            if !self.roles.contains(r) {
                self.roles.push(r.clone());
            }
        }
        for p in &other.policies {
            if !self.policies.contains(p) {
                self.policies.push(p.clone());
            }
        }
        self.anonymous |= other.anonymous;
    }
}

pub fn extract(src: Src, root: Node, symbols: &[Symbol], function_ids: &HashMap<usize, String>, original: &str) -> FileKnowledge {
    let owners = Owners::new(symbols, src.file);
    let server = if original.contains("fastify") {
        "fastify"
    } else if original.contains("from \"hono\"") || original.contains("from 'hono'") {
        "hono"
    } else if original.contains("koa") {
        "koa"
    } else {
        "express"
    };
    let imports_server = ["express", "fastify", "koa", "hono", "@hapi/hapi", "restify"]
        .iter()
        .any(|m| original.contains(&format!("from \"{m}")) || original.contains(&format!("from '{m}")) || original.contains(&format!("require(\"{m}")) || original.contains(&format!("require('{m}")));
    let react_router = original.contains("react-router");
    let mut w = Walker {
        src,
        owners: &owners,
        symbols,
        function_ids,
        server,
        imports_server,
        imports: HashMap::new(),
        router_auth: HashMap::new(),
        react_router,
        out: FileKnowledge::default(),
    };
    w.collect_imports(root);
    w.file_level();
    w.walk(root, None);
    w.apply_router_auth();
    w.out
}

/// Project path of a relative module specifier, without extension: `./routes/orders` from
/// `src/app.ts` → `src/routes/orders`. `None` for packages.
fn module_path(file: &str, spec: &str) -> Option<String> {
    if !spec.starts_with('.') {
        return None;
    }
    let mut parts: Vec<&str> = file.split('/').collect();
    parts.pop();
    for seg in spec.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    let joined = parts.join("/");
    // `./orders.js` in ESM TypeScript names `orders.ts`.
    let name_at = joined.rfind('/').map(|i| i + 1).unwrap_or(0);
    let stem = match joined[name_at..].find('.') {
        Some(dot) => joined[..name_at + dot].to_owned(),
        None => joined,
    };
    Some(stem)
}

/// Middleware or guard names that protect a route (`requireAuth`, `passport.authenticate('jwt')`,
/// `AuthGuard('jwt')`, `JwtAuthGuard`), and the roles they name (`requireRole('admin')`).
fn auth_of(src: Src, node: Node) -> Option<Middleware> {
    let (name, args) = match node.kind() {
        "call_expression" => (
            node.child_by_field_name("function").map(|f| src.norm(f)).unwrap_or_default(),
            node.child_by_field_name("arguments").map(named_children).unwrap_or_default(),
        ),
        "identifier" | "member_expression" => (src.norm(node), Vec::new()),
        _ => return None,
    };
    let lower = name.to_ascii_lowercase();
    let protects = ["auth", "login", "jwt", "protect", "verifytoken", "guard", "role", "permission", "admin", "session", "bearer"]
        .iter()
        .any(|w| lower.contains(w));
    if !protects {
        return None;
    }
    let mut m = Middleware::default();
    if ["public", "anonymous", "optional"].iter().any(|w| lower.contains(w)) {
        m.anonymous = true;
        return Some(m);
    }
    let names_roles = ["role", "permission", "authorize", "admin", "can"].iter().any(|w| lower.contains(w));
    if names_roles {
        for a in &args {
            collect_strings(src, *a, &mut m.roles);
        }
    }
    if lower.contains("admin") && m.roles.is_empty() {
        m.roles.push("admin".into());
    }
    m.policies.push(name.rsplit('.').next().unwrap_or(&name).to_owned());
    Some(m)
}

/// String literals in a node: `'admin'`, `['admin', 'ops']`.
fn collect_strings(src: Src, node: Node, out: &mut Vec<String>) {
    if let Some(s) = string_of(src, node) {
        if !out.contains(&s) {
            out.push(s);
        }
        return;
    }
    for c in named_children(node) {
        collect_strings(src, c, out);
    }
}

/// The value of `key` in an object literal.
fn field<'t>(src: Src, object: Node<'t>, key: &str) -> Option<Node<'t>> {
    children_of_kind(object, "pair")
        .into_iter()
        .find(|p| p.child_by_field_name("key").is_some_and(|k| key_text(src, k) == key))
        .and_then(|p| p.child_by_field_name("value"))
}

fn key_text(src: Src, key: Node) -> String {
    string_of(src, key).unwrap_or_else(|| src.norm(key))
}

/// The last string literal passed to `name(...)` in `text`: `program.command('add <x>')`.
fn call_string(text: &str, name: &str) -> Option<String> {
    let at = text.rfind(&format!("{name}("))? + name.len() + 1;
    let rest = text[at..].trim_start();
    let quote = rest.chars().next().filter(|c| matches!(c, '\'' | '"' | '`'))?;
    let end = rest[1..].find(quote)?;
    Some(rest[1..1 + end].to_owned())
}

/// A NestJS controller's prefix and guards, passed to its methods.
struct ClassCtx {
    prefix: Option<String>,
    guarded: bool,
    auth: Middleware,
    /// Built by a DI container (`@Injectable`, `@Controller`, Angular `@Component`, ...):
    /// constructor parameters are injected.
    injectable: bool,
}

impl<'a> Walker<'a> {
    /// Facts the file path alone shows: test files, Next.js route handlers and API pages.
    fn file_level(&mut self) {
        let file = self.src.file;
        let module = self.symbols.iter().find(|s| s.kind == SymbolKind::Module && s.parent.is_none()).map(|s| s.id.clone());
        let lower = file.to_ascii_lowercase();
        if (lower.contains(".test.") || lower.contains(".spec.") || lower.contains("/__tests__/") || lower.contains("/e2e/"))
            && let Some(m) = &module
        {
            self.out.symbol_mut(m).test = true;
        }
        // Next.js: `app/**/route.ts` exports one function per HTTP method; `pages/api/**` exports a handler.
        let parts: Vec<&str> = file.split('/').collect();
        let stem = parts.last().map(|p| p.split('.').next().unwrap_or(p)).unwrap_or("");
        if let Some(app) = parts.iter().position(|p| *p == "app")
            && stem == "route"
        {
            let path = route_path(&parts[app + 1..parts.len() - 1]);
            for s in self.symbols.iter().filter(|s| s.kind == SymbolKind::Function && s.parent == module) {
                if matches!(s.name.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS")
                    && let Some(span) = s.origins.first()
                {
                    self.out.routes.push(RouteFact {
                        handler: s.id.clone(),
                        handler_ref: None,
                        method: s.name.clone(),
                        path: path.clone(),
                        framework: "nextjs".into(),
                        span: span.clone(),
                        router: None,
                    });
                }
            }
        } else if let Some(app) = parts.iter().position(|p| *p == "app").filter(|_| stem == "page") {
            // `app/**/page.tsx`: a page.
            let path = route_path(&parts[app + 1..parts.len() - 1]);
            if let Some(page) = self.symbols.iter().find(|s| s.flavors.iter().any(|f| f == "default-export") && matches!(s.kind, SymbolKind::Function | SymbolKind::Class))
                && let Some(span) = page.origins.first()
            {
                self.out.routes.push(RouteFact { handler: page.id.clone(), handler_ref: None, method: "PAGE".into(), path, framework: "nextjs".into(), span: span.clone(), router: None });
            }
        } else if let Some(pages) = parts.iter().position(|p| *p == "pages").filter(|i| parts.get(i + 1) != Some(&"api") && !stem.starts_with('_')) {
            // `pages/orders/[id].tsx`: a page (`pages/api/**` are handlers, `_app` is not a page).
            let mut segs: Vec<&str> = parts[pages + 1..parts.len() - 1].to_vec();
            if stem != "index" {
                segs.push(stem);
            }
            let path = route_path(&segs);
            if let Some(page) = self.symbols.iter().find(|s| s.flavors.iter().any(|f| f == "default-export") && matches!(s.kind, SymbolKind::Function | SymbolKind::Class))
                && let Some(span) = page.origins.first()
            {
                self.out.routes.push(RouteFact { handler: page.id.clone(), handler_ref: None, method: "PAGE".into(), path, framework: "nextjs".into(), span: span.clone(), router: None });
            }
        } else if let Some(pages) = parts.windows(2).position(|w| w == ["pages", "api"]) {
            let mut segs: Vec<&str> = parts[pages + 1..parts.len() - 1].to_vec();
            if stem != "index" {
                segs.push(stem);
            }
            let path = route_path(&segs);
            if let Some(handler) = self.symbols.iter().find(|s| s.kind == SymbolKind::Function && s.flavors.iter().any(|f| f == "default-export"))
                && let Some(span) = handler.origins.first()
            {
                self.out.routes.push(RouteFact { handler: handler.id.clone(), handler_ref: None, method: "ANY".into(), path, framework: "nextjs".into(), span: span.clone(), router: None });
            }
        }
    }

    fn walk(&mut self, node: Node, class: Option<&ClassCtx>) {
        match node.kind() {
            "class_declaration" | "class" | "abstract_class_declaration" => {
                let ctx = self.class(node);
                for child in named_children(node) {
                    self.walk(child, Some(&ctx));
                }
                return;
            }
            "method_definition" => self.method(node, class),
            "required_parameter" | "optional_parameter" => self.injection(node, class),
            "call_expression" => self.call(node),
            "jsx_opening_element" | "jsx_self_closing_element" => self.jsx(node),
            "object" if self.react_router => self.route_object(node),
            "member_expression" | "subscript_expression" => self.env(node),
            _ => {}
        }
        for child in named_children(node) {
            self.walk(child, class);
        }
    }

    fn class(&mut self, node: Node) -> ClassCtx {
        let src = self.src;
        let decorators = class_decorators(src, node);
        let id = self.owners.declared_at(node.start_byte(), node.end_byte(), SymbolKind::is_type).map(str::to_owned);
        let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
        let mut ctx = ClassCtx { prefix: None, guarded: false, auth: Middleware::default(), injectable: false };
        for d in &decorators {
            if let Some(m) = self.decorator_auth(d) {
                ctx.auth.merge(&m);
            }
            if matches!(d.name.as_str(), "Injectable" | "Controller" | "Component" | "Directive" | "Pipe" | "Resolver" | "Gateway" | "Processor") {
                ctx.injectable = true;
            }
            match d.name.as_str() {
                "Controller" => ctx.prefix = Some(d.args.first().and_then(|a| string_of(src, *a)).unwrap_or_default()),
                "UseGuards" => ctx.guarded = true,
                "Module" => {
                    if let (Some(id), Some(options)) = (&id, d.args.first()) {
                        self.nest_module(id, *options);
                    }
                }
                "Entity" => {
                    if let Some(id) = &id {
                        self.out.entities.push(EntityFact {
                            owner: id.clone(),
                            entity: name.clone(),
                            property: None,
                            framework: "typeorm".into(),
                            table: d.args.first().and_then(|a| string_of(src, *a)),
                            span: src.span(node),
                        });
                    }
                }
                "Catch" => {
                    let handler = id.as_ref().and_then(|cid| self.symbols.iter().find(|s| s.parent.as_deref() == Some(cid) && s.name == "catch"));
                    if let Some(h) = handler {
                        let exceptions: Vec<String> = d.args.iter().map(|a| src.norm(*a)).collect();
                        let exception = if exceptions.is_empty() { "*".to_owned() } else { exceptions.join(" | ") };
                        self.out.handlers.push(GlobalHandlerFact { handler: h.id.clone(), exception, framework: "nestjs".into(), span: src.span(node) });
                    }
                }
                _ => {}
            }
        }
        ctx
    }

    /// `@Module({ providers: [Service, { provide: "TOKEN", useClass: Impl }] })`.
    fn nest_module(&mut self, owner: &str, options: Node) {
        let src = self.src;
        for pair in children_of_kind(options, "pair") {
            let key = pair.child_by_field_name("key").map(|k| src.norm(k)).unwrap_or_default();
            if key != "providers" {
                continue;
            }
            let Some(list) = pair.child_by_field_name("value").filter(|v| v.kind() == "array") else { continue };
            for item in named_children(list) {
                match item.kind() {
                    "identifier" => self.out.di.push(DiFact {
                        owner: owner.to_owned(),
                        service: src.norm(item),
                        implementation: None,
                        lifetime: "class".into(),
                        token: None,
                        framework: "nestjs".into(),
                        span: src.span(item),
                    }),
                    "object" => {
                        let field = |name: &str| {
                            children_of_kind(item, "pair")
                                .into_iter()
                                .find(|p| p.child_by_field_name("key").is_some_and(|k| src.norm(k) == name))
                                .and_then(|p| p.child_by_field_name("value"))
                        };
                        let Some(provide) = field("provide") else { continue };
                        let token = string_of(src, provide);
                        let service = token.clone().unwrap_or_else(|| src.norm(provide));
                        let (implementation, lifetime) = if let Some(c) = field("useClass") {
                            (Some(src.norm(c)), "class")
                        } else if let Some(e) = field("useExisting") {
                            (Some(src.norm(e)), "existing")
                        } else if field("useFactory").is_some() {
                            (None, "factory")
                        } else {
                            (None, "value")
                        };
                        self.out.di.push(DiFact {
                            owner: owner.to_owned(),
                            service,
                            implementation,
                            lifetime: lifetime.into(),
                            token,
                            framework: "nestjs".into(),
                            span: src.span(item),
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    /// `constructor(@Inject("TOKEN") private readonly gateway: PaymentGateway)`, and plain typed
    /// constructor parameters of a class the container builds (`constructor(private orders: OrdersService)`).
    fn injection(&mut self, node: Node, class: Option<&ClassCtx>) {
        let src = self.src;
        let Some(ty) = node.child_by_field_name("type").map(|t| crate::syntax::type_text(src, t)) else { return };
        let Some(owner) = self.owners.function_at(node.start_byte()).map(str::to_owned) else { return };
        let inject = children_of_kind(node, "decorator").into_iter().filter_map(|d| decorator(src, d)).find(|d| d.name == "Inject");
        let token = match inject {
            Some(inject) => match inject.args.first().and_then(|a| string_of(src, *a)) {
                Some(token) => Some(token),
                None => return,
            },
            None => {
                // Only constructor parameters of a container-built class are injected.
                let in_constructor = node
                    .parent()
                    .and_then(|l| l.parent())
                    .filter(|m| m.kind() == "method_definition")
                    .and_then(|m| m.child_by_field_name("name"))
                    .is_some_and(|n| src.norm(n) == "constructor");
                if !(in_constructor && class.is_some_and(|c| c.injectable)) {
                    return;
                }
                None
            }
        };
        let service = crate::syntax::bare_type_name(&ty);
        if token.is_none() && !service.chars().next().is_some_and(char::is_uppercase) {
            return;
        }
        self.out.di.push(DiFact {
            owner,
            service,
            implementation: None,
            lifetime: "inject".into(),
            token,
            framework: "nestjs".into(),
            span: src.span(node),
        });
    }

    /// Authorization a decorator declares: `@UseGuards(JwtAuthGuard)`, `@Roles('admin')`,
    /// `@Public()`, `@Authorize()`.
    fn decorator_auth(&self, d: &Decorator) -> Option<Middleware> {
        let src = self.src;
        let mut m = Middleware::default();
        match d.name.as_str() {
            "UseGuards" => {
                for a in &d.args {
                    let text = src.norm(*a);
                    m.policies.push(text.split('(').next().unwrap_or(&text).rsplit('.').next().unwrap_or(&text).to_owned());
                }
            }
            "Roles" | "HasRoles" | "RequireRoles" | "Permissions" | "RequirePermissions" => {
                for a in &d.args {
                    collect_strings(src, *a, &mut m.roles);
                    if string_of(src, *a).is_none() && named_children(*a).is_empty() {
                        // `@Roles(Role.Admin)`: the enum member name.
                        let text = src.norm(*a);
                        m.roles.push(text.rsplit('.').next().unwrap_or(&text).to_owned());
                    }
                }
            }
            "Public" | "AllowAnonymous" | "SkipAuth" | "IsPublic" => m.anonymous = true,
            "Authorize" | "Auth" | "Authenticated" => m.policies.push(d.name.clone()),
            _ => return None,
        }
        Some(m)
    }

    fn auth(&mut self, owner: &str, m: &Middleware, framework: &str, node: Node) {
        if m.is_empty() {
            return;
        }
        self.out.auth.push(AuthFact {
            owner: owner.to_owned(),
            roles: m.roles.clone(),
            policies: m.policies.clone(),
            anonymous: m.anonymous,
            framework: framework.to_owned(),
            span: self.src.span(node),
        });
    }

    /// `router.use(requireAuth)` protects every route of `router` in this file.
    fn apply_router_auth(&mut self) {
        let mut facts = Vec::new();
        for r in &self.out.routes {
            if let Some(m) = r.router.as_ref().and_then(|router| self.router_auth.get(router)) {
                facts.push(AuthFact {
                    owner: r.handler.clone(),
                    roles: m.roles.clone(),
                    policies: m.policies.clone(),
                    anonymous: m.anonymous,
                    framework: r.framework.clone(),
                    span: r.span.clone(),
                });
            }
        }
        self.out.auth.extend(facts);
    }

    /// `import orders from "./routes/orders"`, `const orders = require("./routes/orders")`.
    fn collect_imports(&mut self, node: Node) {
        let src = self.src;
        match node.kind() {
            "import_statement" => {
                let spec = node.child_by_field_name("source").map(|s| crate::syntax::string_value(src, s)).unwrap_or_default();
                if let Some(path) = module_path(src.file, &spec) {
                    let mut names = Vec::new();
                    for clause in children_of_kind(node, "import_clause") {
                        collect_import_names(src, clause, &mut names);
                    }
                    for n in names {
                        self.imports.insert(n, path.clone());
                    }
                }
                return;
            }
            "variable_declarator" => {
                if let (Some(name), Some(value)) = (node.child_by_field_name("name"), node.child_by_field_name("value"))
                    && name.kind() == "identifier"
                    && let Some(path) = require_path(src, value)
                {
                    self.imports.insert(src.norm(name), path);
                }
            }
            _ => {}
        }
        for c in named_children(node) {
            self.collect_imports(c);
        }
    }

    /// A mount target as written: an imported router names its module.
    fn mount_target(&self, node: Node) -> String {
        let src = self.src;
        if node.kind() == "identifier"
            && let Some(path) = self.imports.get(&src.norm(node))
        {
            return format!("module:{path}");
        }
        if let Some(path) = require_path(src, node) {
            return format!("module:{path}");
        }
        src.norm(node)
    }

    /// The function a callback argument runs: its own Blueprint, a named function, or the function
    /// an expression-bodied arrow calls (`() => save(order)`).
    fn callback_target(&self, node: Node) -> Option<String> {
        if let (Some(id), _) = self.handler_of(Some(node)) {
            return Some(id);
        }
        if crate::syntax::is_function_value(node.kind())
            && let Some(body) = node.child_by_field_name("body").filter(|b| b.kind() == "call_expression")
            && let Some(f) = body.child_by_field_name("function")
        {
            return self.handler_of(Some(f)).0;
        }
        None
    }

    /// `<button onClick={save}>Save</button>`: a UI entry; `<Route path="/x" element={<X/>}/>`: a page.
    fn jsx<'t>(&mut self, node: Node<'t>) {
        fn value_expr(v: Node<'_>) -> Node<'_> {
            if v.kind() == "jsx_expression" { named_children(v).into_iter().next().unwrap_or(v) } else { v }
        }
        let src = self.src;
        let tag = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
        let attrs: Vec<(String, Option<Node<'t>>)> = children_of_kind(node, "jsx_attribute")
            .into_iter()
            .filter_map(|a| {
                let kids = named_children(a);
                let name = kids.first().map(|n| src.norm(*n))?;
                Some((name, kids.get(1).copied()))
            })
            .collect();
        if self.react_router && tag == "Route" {
            let path = attrs.iter().find(|(n, _)| n == "path").and_then(|(_, v)| v.map(value_expr)).and_then(|v| string_of(src, v));
            let component = attrs
                .iter()
                .find(|(n, _)| matches!(n.as_str(), "element" | "component" | "Component"))
                .and_then(|(_, v)| v.map(value_expr))
                .and_then(|v| jsx_component_name(src, v));
            if let Some(path) = path {
                self.page(&path, component, "react-router", node);
            }
            return;
        }
        for (name, value) in &attrs {
            if !matches!(name.as_str(), "onClick" | "onSubmit" | "onPress" | "onDoubleClick" | "onTap") {
                continue;
            }
            let Some(value) = value.map(value_expr) else { continue };
            let Some(handler) = self.callback_target(value) else { continue };
            let text = node
                .parent()
                .filter(|p| p.kind() == "jsx_element")
                .map(|el| children_of_kind(el, "jsx_text").into_iter().map(|t| src.norm(t)).collect::<Vec<_>>().join(" "))
                .map(|t| t.trim().to_owned())
                .filter(|t| !t.is_empty());
            let what = if name == "onSubmit" { "submit" } else { "click" };
            let label = match text {
                Some(t) => format!("{what} \"{t}\""),
                None => format!("{what} <{tag}>"),
            };
            self.out.entry(&handler, EntryKind::Ui, label);
        }
    }

    /// `{ path: "/orders", element: <Orders /> }` in a React Router configuration.
    fn route_object(&mut self, node: Node) {
        let src = self.src;
        let Some(path) = field(src, node, "path").and_then(|p| string_of(src, p)) else { return };
        let component = ["element", "Component", "component"].iter().find_map(|k| field(src, node, k)).and_then(|v| jsx_component_name(src, v));
        if component.is_some() {
            self.page(&path, component, "react-router", node);
        }
    }

    fn page(&mut self, path: &str, component: Option<String>, framework: &str, node: Node) {
        let src = self.src;
        let owner = self.owners.function_at(node.start_byte()).map(str::to_owned).unwrap_or_default();
        let local = component.as_ref().and_then(|c| {
            self.symbols.iter().find(|s| &s.name == c && matches!(s.kind, SymbolKind::Function | SymbolKind::Class | SymbolKind::Method)).map(|s| s.id.clone())
        });
        let path = if path.starts_with('/') { path.to_owned() } else { format!("/{path}") };
        self.out.routes.push(RouteFact {
            handler: local.unwrap_or(owner),
            handler_ref: component,
            method: "PAGE".into(),
            path,
            framework: framework.into(),
            span: src.span(node),
            router: None,
        });
    }

    /// XState `createMachine({ id, initial, states: { idle: { on: { FETCH: 'loading' } } } })`.
    fn xstate(&mut self, node: Node, config: Node) {
        let src = self.src;
        let owner = self.owners.function_at(node.start_byte()).map(str::to_owned).unwrap_or_default();
        let declared = node
            .parent()
            .filter(|p| p.kind() == "variable_declarator")
            .and_then(|p| p.child_by_field_name("name"))
            .map(|n| src.norm(n));
        let name = field(src, config, "id").and_then(|i| string_of(src, i)).or(declared).unwrap_or_else(|| "machine".into());
        let mut machine = StateMachineFact {
            owner,
            name,
            framework: "xstate".into(),
            states: Vec::new(),
            initial: field(src, config, "initial").and_then(|i| string_of(src, i)),
            transitions: Vec::new(),
            triggers: Vec::new(),
            span: src.span(node),
        };
        let root_id = field(src, config, "id").and_then(|i| string_of(src, i)).unwrap_or_default();
        xstate_states(src, config, "", &root_id, &mut machine);
        // Root-level `on`: from any state.
        if let Some(on) = field(src, config, "on").filter(|o| o.kind() == "object") {
            xstate_on(src, on, "*", "", &root_id, &mut machine);
        }
        if !machine.states.is_empty() {
            self.out.state_machines.push(machine);
        }
    }

    fn method(&mut self, node: Node, class: Option<&ClassCtx>) {
        let src = self.src;
        let Some(id) = self.owners.declared_at(node.start_byte(), node.end_byte(), |k| matches!(k, SymbolKind::Method | SymbolKind::Function)).map(str::to_owned) else {
            return;
        };
        let decorators = member_decorators(src, node);
        let guarded = class.is_some_and(|c| c.guarded) || decorators.iter().any(|d| d.name == "UseGuards" || AUTH_MARKERS.contains(&d.name.as_str()));
        let prefix = class.and_then(|c| c.prefix.clone());
        let mut auth = class.map(|c| c.auth.clone()).unwrap_or_default();
        for d in &decorators {
            if let Some(m) = self.decorator_auth(d) {
                if m.anonymous {
                    auth = Middleware { anonymous: true, ..Middleware::default() };
                } else if !auth.anonymous {
                    auth.merge(&m);
                }
            }
        }
        let routed = prefix.is_some() && decorators.iter().any(|d| NEST_VERBS.iter().any(|(n, _)| *n == d.name));
        if routed {
            self.auth(&id, &auth, "nestjs", node);
        }
        for d in &decorators {
            if let (Some(prefix), Some((_, verb))) = (&prefix, NEST_VERBS.iter().find(|(n, _)| *n == d.name)) {
                let path = d.args.first().and_then(|a| string_of(src, *a)).unwrap_or_default();
                self.out.routes.push(RouteFact {
                    handler: id.clone(),
                    handler_ref: None,
                    method: (*verb).into(),
                    path: join(prefix, &path),
                    framework: "nestjs".into(),
                    span: src.span(node),
                    router: None,
                });
            } else {
                let arg = d.args.first().map(|a| string_of(src, *a).unwrap_or_else(|| src.norm(*a))).unwrap_or_default();
                let arg = if arg.is_empty() { String::new() } else { format!(" {arg}") };
                match d.name.as_str() {
                    "MessagePattern" | "EventPattern" | "Process" | "RabbitSubscribe" | "SqsMessageHandler" => {
                        self.out.entry(&id, EntryKind::Message, format!("message @{}{arg}", d.name))
                    }
                    "Cron" | "Interval" | "Timeout" => self.out.entry(&id, EntryKind::Job, format!("scheduled @{}{arg}", d.name)),
                    "OnEvent" | "SubscribeMessage" => self.out.entry(&id, EntryKind::EventHandler, format!("event handler @{}{arg}", d.name)),
                    _ => {}
                }
            }
        }
        if guarded && prefix.is_some() && !auth.anonymous {
            self.out.capability(&id, "AUTH_REQUIRED");
        }
    }

    /// `process.env.X`, `process.env["X"]`, `import.meta.env.X`.
    fn env(&mut self, node: Node) {
        let src = self.src;
        let Some(object) = node.child_by_field_name("object") else { return };
        let base = src.norm(object);
        if base != "process.env" && base != "import.meta.env" {
            return;
        }
        let key = match node.kind() {
            "member_expression" => node.child_by_field_name("property").map(|p| src.norm(p)),
            _ => node.child_by_field_name("index").and_then(|i| string_of(src, i)),
        };
        let (Some(key), Some(owner)) = (key, self.owners.function_at(node.start_byte()).map(str::to_owned)) else { return };
        self.out.config.push(ConfigFact { owner, key, source: "env".into(), default: None, span: src.span(node) });
    }

    fn call(&mut self, node: Node) {
        let src = self.src;
        let Some(function) = node.child_by_field_name("function") else { return };
        let args: Vec<Node> = node.child_by_field_name("arguments").map(named_children).unwrap_or_default();
        let (receiver, name) = match function.kind() {
            "member_expression" => (
                function.child_by_field_name("object").map(|o| src.norm(o)).unwrap_or_default(),
                function.child_by_field_name("property").map(|p| src.norm(p)).unwrap_or_default(),
            ),
            "identifier" => (String::new(), src.norm(function)),
            _ => return,
        };
        let Some(owner) = self.owners.function_at(node.start_byte()).map(str::to_owned) else { return };
        for tag in call_capabilities(&name, &receiver) {
            self.out.capability(&owner, tag);
        }
        let recv = receiver.to_ascii_lowercase();
        let recv_last = recv.rsplit('.').next().unwrap_or("").trim_start_matches('_').to_owned();
        let first_string = args.first().and_then(|a| string_of(src, *a));

        // Tests: `it("does x", () => { ... })`.
        if receiver.is_empty() || matches!(receiver.as_str(), "it" | "test" | "describe") {
            let base = if receiver.is_empty() { name.as_str() } else { receiver.as_str() };
            if matches!(base, "it" | "test" | "specify") {
                if let Some(callback) = args.iter().find(|a| crate::syntax::is_function_value(a.kind()))
                    && let Some(id) = self.function_ids.get(&callback.start_byte()).cloned()
                {
                    self.out.symbol_mut(&id).test = true;
                    let title = first_string.clone().unwrap_or_default();
                    self.out.entry(&id, EntryKind::Test, format!("test: {title}"));
                }
                return;
            }
        }

        // Server routes: `app.get("/x", handler)`, `router.post("/x", auth, handler)`. `api` and
        // `server` are also common names of HTTP clients (`api.get("/orders")` on an axios
        // instance), so they count only in files that import a server framework.
        let routerish = matches!(recv_last.as_str(), "app" | "router" | "fastify")
            || recv_last.ends_with("router")
            || self.imports_server && matches!(recv_last.as_str(), "server" | "routes" | "api");
        // A route always has a handler after the path; a client call has a config object or
        // nothing. Fastify route options carry it as `{ schema, handler }`; a spread
        // (`...middlewares`) hides it.
        let handler_arg = args.last().copied().filter(|_| args.len() >= 2).and_then(|a| match a.kind() {
            "object" => options_handler(src, a).map(Some),
            "spread_element" => Some(None),
            k if crate::syntax::is_function_value(k) || matches!(k, "identifier" | "member_expression" | "call_expression") => Some(Some(a)),
            _ => None,
        });
        if routerish
            && ROUTE_VERBS.contains(&name.as_str())
            && let Some(handler_arg) = handler_arg
            && let Some(path) = first_string.as_ref().filter(|p| p.starts_with('/') || p.as_str() == "*")
        {
            let method = match name.as_str() {
                "del" => "DELETE".to_owned(),
                "all" => "ANY".to_owned(),
                other => other.to_uppercase(),
            };
            let (handler, handler_ref) = self.handler_of(handler_arg);
            let handler = handler.unwrap_or_else(|| owner.clone());
            // Middlewares between the path and the handler: `app.get('/x', requireRole('admin'), h)`.
            let mut auth = Middleware::default();
            for m in args.iter().skip(1).take(args.len().saturating_sub(2)) {
                if let Some(a) = auth_of(src, *m) {
                    auth.merge(&a);
                }
            }
            self.auth(&handler, &auth, self.server, node);
            self.out.routes.push(RouteFact {
                handler,
                handler_ref,
                method,
                path: path.clone(),
                framework: self.server.into(),
                span: src.span(node),
                router: Some(receiver.clone()),
            });
            return;
        }
        if routerish && name == "use" {
            // `app.use("/api", router)`: a mount; `app.use((err, req, res, next) => ...)`: an error handler.
            if let (Some(prefix), Some(target)) = (first_string.as_ref(), args.last().filter(|_| args.len() >= 2)) {
                self.out.prefixes.push(RoutePrefix {
                    kind: "mount".into(),
                    target: self.mount_target(*target),
                    prefix: prefix.clone(),
                    framework: self.server.into(),
                    span: src.span(node),
                    router: Some(receiver.clone()),
                });
            } else if first_string.is_none() {
                // `router.use(requireAuth)`: every route of this router is protected.
                for a in &args {
                    if let Some(m) = auth_of(src, *a) {
                        self.router_auth.entry(receiver.clone()).or_default().merge(&m);
                    }
                }
            }
            if let Some(f) = args.last().filter(|a| crate::syntax::is_function_value(a.kind()))
                && parameters(src, *f).len() == 4
            {
                let handler = self.handler_of(Some(*f)).0.unwrap_or_else(|| owner.clone());
                self.out.handlers.push(GlobalHandlerFact { handler, exception: "*".into(), framework: self.server.into(), span: src.span(node) });
            }
            return;
        }
        match (recv_last.as_str(), name.as_str()) {
            ("app" | "server" | "fastify" | "httpserver", "listen") | ("nestfactory", "create") => {
                self.out.entry(&owner, EntryKind::ServerStart, "server start");
            }
            (_, "setGlobalPrefix") => {
                if let Some(prefix) = first_string {
                    self.out.prefixes.push(RoutePrefix { kind: "global".into(), target: String::new(), prefix, framework: "nestjs".into(), span: src.span(node), router: None });
                }
            }
            (_, "setErrorHandler") => {
                self.out.handlers.push(GlobalHandlerFact { handler: owner.clone(), exception: "*".into(), framework: self.server.into(), span: src.span(node) });
            }
            (_, "get") if recv_last.contains("config") => {
                self.out.config.push(ConfigFact { owner: owner.clone(), key: first_string.unwrap_or_default(), source: "configuration".into(), default: None, span: src.span(node) });
            }
            (_, "create" | "createMany" | "update" | "updateMany" | "upsert" | "delete" | "deleteMany") if recv.contains("prisma") => {
                self.mutation(&owner, &receiver, "database", node);
            }
            (_, "save" | "insert" | "update" | "delete" | "remove" | "softDelete") if recv_last.contains("repo") => {
                self.mutation(&owner, &receiver, "database", node);
            }
            ("localstorage" | "sessionstorage", "setItem" | "removeItem" | "clear") => self.mutation(&owner, &receiver, "storage", node),
            _ => {}
        }

        self.entries_and_machines(node, &receiver, &name, &args);

        // HTTP clients.
        let fetchish = receiver.is_empty() && matches!(name.as_str(), "fetch" | "$fetch" | "ofetch" | "useFetch" | "axios")
            || receiver == "window" && name == "fetch";
        let clientish = ["axios", "http", "api", "client", "ky", "instance", "superagent", "request"].iter().any(|c| recv_last.contains(c))
            && !routerish
            && CLIENT_VERBS.contains(&name.as_str());
        if fetchish || clientish {
            let Some(url_node) = args.first().copied() else { return };
            let method = if clientish && name != "request" {
                name.to_uppercase()
            } else {
                // `fetch(url, { method: "POST" })`.
                args.get(1)
                    .and_then(|o| children_of_kind(*o, "pair").into_iter().find(|p| p.child_by_field_name("key").is_some_and(|k| src.norm(k) == "method")))
                    .and_then(|p| p.child_by_field_name("value"))
                    .and_then(|v| string_of(src, v))
                    .map(|m| m.to_uppercase())
                    .unwrap_or_else(|| "GET".into())
            };
            let url = self.url_of(url_node);
            let client = if fetchish { name.trim_start_matches('$').to_owned() } else { recv_last.clone() };
            self.out.http_calls.push(HttpCallFact { owner, method, url, client, span: src.span(node) });
        }
    }

    /// Commands (commander, yargs), jobs (`setInterval`, cron), message consumers (KafkaJS,
    /// amqplib) and XState machines.
    fn entries_and_machines(&mut self, node: Node, receiver: &str, name: &str, args: &[Node]) {
        let src = self.src;
        let recv = receiver.to_ascii_lowercase();
        match name {
            // commander: `program.command('add <item>').action(handler)`.
            "action" if receiver.contains(".command(") || receiver.starts_with("command(") => {
                if let (Some(cmd), Some(h)) = (call_string(receiver, "command"), args.first().and_then(|a| self.callback_target(*a))) {
                    let word = cmd.split_whitespace().next().unwrap_or(&cmd).to_owned();
                    self.out.entry(&h, EntryKind::Command, format!("command {word}"));
                }
            }
            // yargs: `.command('serve [port]', 'start the server', builder, handler)`.
            "command" if args.len() >= 3 => {
                if let (Some(cmd), Some(h)) = (args.first().and_then(|a| string_of(src, *a)), args.last().and_then(|a| self.callback_target(*a))) {
                    let word = cmd.split_whitespace().next().unwrap_or(&cmd).to_owned();
                    self.out.entry(&h, EntryKind::Command, format!("command {word}"));
                }
            }
            "setInterval" if receiver.is_empty() || recv == "window" || recv == "globalthis" => {
                if let Some(h) = args.first().and_then(|a| self.callback_target(*a)) {
                    let every = args.get(1).map(|a| src.norm(*a)).unwrap_or_default();
                    self.out.entry(&h, EntryKind::Job, format!("every {every} ms"));
                }
            }
            // node-cron `cron.schedule('0 * * * *', fn)`, `new CronJob(...)` is a `new_expression`.
            "schedule" if recv.contains("cron") || recv.contains("schedule") => {
                if let (Some(expr), Some(h)) = (args.first().and_then(|a| string_of(src, *a)), args.get(1).and_then(|a| self.callback_target(*a))) {
                    self.out.entry(&h, EntryKind::Job, format!("cron {expr}"));
                }
            }
            // KafkaJS `consumer.run({ eachMessage: async ({ message }) => { ... } })`.
            "run" if recv.contains("consumer") => {
                if let Some(options) = args.first().filter(|a| a.kind() == "object") {
                    for key in ["eachMessage", "eachBatch"] {
                        if let Some(h) = field(src, *options, key).and_then(|v| self.callback_target(v)) {
                            self.out.entry(&h, EntryKind::Message, format!("Kafka consumer ({key})"));
                        }
                    }
                }
            }
            // amqplib `channel.consume('orders', fn)`.
            "consume" if recv.contains("channel") || recv == "ch" || recv.contains("queue") => {
                if let Some(h) = args.get(1).and_then(|a| self.callback_target(*a)) {
                    let queue = args.first().and_then(|a| string_of(src, *a)).unwrap_or_default();
                    self.out.entry(&h, EntryKind::Message, format!("queue {queue}").trim().to_owned());
                }
            }
            "createMachine" | "Machine" | "createActor" if args.first().is_some_and(|a| a.kind() == "object") => {
                if let Some(config) = args.first() {
                    self.xstate(node, *config);
                }
            }
            _ => {}
        }
    }

    /// The symbol of a route handler argument (an inline function with a Blueprint, or a function
    /// of this file named like it) and, for a name, the name as written for `nodex-core` to resolve
    /// over the project (`ordersController.list` from another module).
    fn handler_of(&self, arg: Option<Node>) -> (Option<String>, Option<String>) {
        let Some(arg) = arg else { return (None, None) };
        if let Some(id) = self.function_ids.get(&arg.start_byte()) {
            return (Some(id.clone()), None);
        }
        if matches!(arg.kind(), "identifier" | "member_expression") {
            let text = self.src.norm(arg);
            let name = text.rsplit('.').next().unwrap_or(&text);
            let local = self
                .symbols
                .iter()
                .find(|s| s.name == name && matches!(s.kind, SymbolKind::Function | SymbolKind::Method))
                .map(|s| s.id.clone());
            return (local, Some(text));
        }
        (None, None)
    }

    /// A URL as written; `p:<index>` when it is a parameter of the owner (an HTTP wrapper); `${x}`
    /// (dynamic) for any other expression.
    fn url_of(&self, node: Node) -> String {
        let src = self.src;
        if let Some(s) = string_of(src, node) {
            return s;
        }
        let text = src.norm(node);
        if node.kind() == "identifier" {
            let mut cur = node.parent();
            while let Some(c) = cur {
                if crate::syntax::is_function_value(c.kind()) || crate::syntax::is_function(c.kind()) || c.kind() == "method_definition" {
                    if let Some(i) = parameters(src, c).iter().position(|(n, _)| *n == text) {
                        return format!("p:{i}");
                    }
                    break;
                }
                cur = c.parent();
            }
        }
        format!("${{{text}}}")
    }

    fn mutation(&mut self, owner: &str, target: &str, kind: &str, node: Node) {
        self.out.mutations.push(MutationFact { owner: owner.to_owned(), target: target.to_owned(), kind: kind.into(), span: self.src.span(node) });
    }
}

/// The `handler` of Fastify route options (`{ schema, handler: list }`).
fn options_handler<'t>(src: Src, options: Node<'t>) -> Option<Node<'t>> {
    children_of_kind(options, "pair")
        .into_iter()
        .find(|p| p.child_by_field_name("key").is_some_and(|k| src.norm(k) == "handler"))
        .and_then(|p| p.child_by_field_name("value"))
}

/// `["users", "[id]"]` → `/users/[id]` (route groups `(admin)` are not part of the URL).
fn route_path(segments: &[&str]) -> String {
    let kept: Vec<&str> = segments.iter().copied().filter(|s| !(s.starts_with('(') && s.ends_with(')')) && !s.is_empty()).collect();
    format!("/{}", kept.join("/"))
}

/// `invoices` + `:id/pay` → `/invoices/:id/pay`.
fn join(prefix: &str, path: &str) -> String {
    let p = prefix.trim().trim_matches('/');
    let s = path.trim().trim_matches('/');
    match (p.is_empty(), s.is_empty()) {
        (true, true) => "/".into(),
        (true, false) => format!("/{s}"),
        (false, true) => format!("/{p}"),
        (false, false) => format!("/{p}/{s}"),
    }
}

/// Local names an import clause binds: `x`, `{ a, b as c }`, `* as ns`.
fn collect_import_names(src: Src, node: Node, out: &mut Vec<String>) {
    match node.kind() {
        "identifier" => out.push(src.norm(node)),
        "import_specifier" => {
            let name = node.child_by_field_name("alias").or_else(|| node.child_by_field_name("name"));
            if let Some(n) = name {
                out.push(src.norm(n));
            }
        }
        _ => {
            for c in named_children(node) {
                collect_import_names(src, c, out);
            }
        }
    }
}

/// `require("./routes/orders")` → the module's project path.
fn require_path(src: Src, node: Node) -> Option<String> {
    if node.kind() != "call_expression" {
        return None;
    }
    let f = node.child_by_field_name("function")?;
    if src.norm(f) != "require" {
        return None;
    }
    let arg = node.child_by_field_name("arguments").map(named_children)?.into_iter().next()?;
    module_path(src.file, &string_of(src, arg)?)
}

/// `<OrdersPage />` or `OrdersPage` → `OrdersPage`.
fn jsx_component_name(src: Src, node: Node) -> Option<String> {
    let node = if node.kind() == "jsx_expression" { named_children(node).into_iter().next()? } else { node };
    match node.kind() {
        "jsx_self_closing_element" => node.child_by_field_name("name").map(|n| src.norm(n)),
        "jsx_element" => node.child_by_field_name("open_tag").and_then(|o| o.child_by_field_name("name")).map(|n| src.norm(n)),
        "identifier" | "member_expression" => Some(src.norm(node)),
        _ => None,
    }
}

/// States of an XState state node, nested ones as `parent.child`.
fn xstate_states(src: Src, node: Node, parent: &str, root_id: &str, m: &mut StateMachineFact) {
    let Some(states) = field(src, node, "states").filter(|s| s.kind() == "object") else { return };
    for pair in children_of_kind(states, "pair") {
        let Some(key) = pair.child_by_field_name("key") else { continue };
        let name = key_text(src, key);
        let full = if parent.is_empty() { name.clone() } else { format!("{parent}.{name}") };
        if !m.states.contains(&full) {
            m.states.push(full.clone());
        }
        let Some(body) = pair.child_by_field_name("value").filter(|v| v.kind() == "object") else { continue };
        if let Some(on) = field(src, body, "on").filter(|o| o.kind() == "object") {
            xstate_on(src, on, &full, parent, root_id, m);
        }
        if let Some(always) = field(src, body, "always") {
            xstate_targets(src, always, &full, parent, root_id, Some("always".into()), m);
        }
        if let Some(after) = field(src, body, "after").filter(|o| o.kind() == "object") {
            for p in children_of_kind(after, "pair") {
                let delay = p.child_by_field_name("key").map(|k| key_text(src, k)).unwrap_or_default();
                if let Some(v) = p.child_by_field_name("value") {
                    xstate_targets(src, v, &full, parent, root_id, Some(format!("after {delay}")), m);
                }
            }
        }
        if let Some(invoke) = field(src, body, "invoke").filter(|o| o.kind() == "object") {
            for (key, trigger) in [("onDone", "done"), ("onError", "error")] {
                if let Some(v) = field(src, invoke, key) {
                    xstate_targets(src, v, &full, parent, root_id, Some(trigger.into()), m);
                }
            }
        }
        xstate_states(src, body, &full, root_id, m);
    }
}

/// `on: { FETCH: 'loading', RETRY: { target: 'loading', guard: 'canRetry' } }`.
fn xstate_on(src: Src, on: Node, from: &str, parent: &str, root_id: &str, m: &mut StateMachineFact) {
    for p in children_of_kind(on, "pair") {
        let trigger = p.child_by_field_name("key").map(|k| key_text(src, k));
        if let Some(v) = p.child_by_field_name("value") {
            xstate_targets(src, v, from, parent, root_id, trigger, m);
        }
    }
}

/// One transition per target of a value: `'x'`, `{ target, guard }`, `[{...}, {...}]`.
fn xstate_targets(src: Src, value: Node, from: &str, parent: &str, root_id: &str, trigger: Option<String>, m: &mut StateMachineFact) {
    match value.kind() {
        "array" => {
            for item in named_children(value) {
                xstate_targets(src, item, from, parent, root_id, trigger.clone(), m);
            }
        }
        "object" => {
            let guard = ["guard", "cond"].iter().find_map(|k| field(src, value, k)).map(|g| string_of(src, g).unwrap_or_else(|| src.norm(g)));
            let Some(target) = field(src, value, "target").and_then(|t| string_of(src, t)) else { return };
            push_transition(m, from, &xstate_resolve(&target, from, parent, root_id), trigger, guard);
        }
        _ => {
            if let Some(target) = string_of(src, value) {
                push_transition(m, from, &xstate_resolve(&target, from, parent, root_id), trigger, None);
            }
        }
    }
}

/// XState target syntax: `sibling`, `.child`, `#machine.state.path`.
fn xstate_resolve(target: &str, from: &str, parent: &str, root_id: &str) -> String {
    if let Some(rest) = target.strip_prefix('#') {
        let rest = rest.strip_prefix(root_id).map(|r| r.trim_start_matches('.')).unwrap_or(rest);
        return rest.to_owned();
    }
    if let Some(child) = target.strip_prefix('.') {
        return format!("{from}.{child}");
    }
    if parent.is_empty() { target.to_owned() } else { format!("{parent}.{target}") }
}

fn push_transition(m: &mut StateMachineFact, from: &str, to: &str, trigger: Option<String>, guard: Option<String>) {
    let t = StateTransition { from: from.to_owned(), to: to.to_owned(), trigger, guard };
    if !m.transitions.contains(&t) {
        m.transitions.push(t);
    }
}
