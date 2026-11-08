//! Knowledge facts of a TypeScript/JavaScript file: Express/Fastify, NestJS and Next.js routes,
//! NestJS dependency injection, HTTP clients (`fetch`, axios, Angular `HttpClient`), environment
//! and configuration reads, Prisma/TypeORM writes and entities, test cases, server start and
//! error handlers.
//!
//! The detection rules were ported from CodeGraph (`crates/core/src/extract/typescript.rs`). This
//! pass walks the same syntax tree as the structure and flow passes, after them.

use std::collections::HashMap;

use nodex_ir::knowledge::{
    ConfigFact, DiFact, EntityFact, EntryKind, FileKnowledge, GlobalHandlerFact, HttpCallFact, MutationFact, RouteFact, RoutePrefix,
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
    out: FileKnowledge,
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
    let mut w = Walker { src, owners: &owners, symbols, function_ids, server, out: FileKnowledge::default() };
    w.file_level();
    w.walk(root, None);
    w.out
}

/// A NestJS controller's prefix and guards, passed to its methods.
struct ClassCtx {
    prefix: Option<String>,
    guarded: bool,
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
                    self.out.routes.push(RouteFact { handler: s.id.clone(), method: s.name.clone(), path: path.clone(), framework: "nextjs".into(), span: span.clone() });
                }
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
                self.out.routes.push(RouteFact { handler: handler.id.clone(), method: "ANY".into(), path, framework: "nextjs".into(), span: span.clone() });
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
            "required_parameter" | "optional_parameter" => self.injection(node),
            "call_expression" => self.call(node),
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
        let mut ctx = ClassCtx { prefix: None, guarded: false };
        for d in &decorators {
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

    /// `constructor(@Inject("TOKEN") private readonly gateway: PaymentGateway)`.
    fn injection(&mut self, node: Node) {
        let src = self.src;
        let Some(inject) = children_of_kind(node, "decorator").into_iter().filter_map(|d| decorator(src, d)).find(|d| d.name == "Inject") else { return };
        let Some(token) = inject.args.first().and_then(|a| string_of(src, *a)) else { return };
        let Some(ty) = node.child_by_field_name("type").map(|t| crate::syntax::type_text(src, t)) else { return };
        let Some(owner) = self.owners.function_at(node.start_byte()).map(str::to_owned) else { return };
        self.out.di.push(DiFact {
            owner,
            service: crate::syntax::bare_type_name(&ty),
            implementation: None,
            lifetime: "inject".into(),
            token: Some(token),
            framework: "nestjs".into(),
            span: src.span(node),
        });
    }

    fn method(&mut self, node: Node, class: Option<&ClassCtx>) {
        let src = self.src;
        let Some(id) = self.owners.declared_at(node.start_byte(), node.end_byte(), |k| matches!(k, SymbolKind::Method | SymbolKind::Function)).map(str::to_owned) else {
            return;
        };
        let decorators = member_decorators(src, node);
        let guarded = class.is_some_and(|c| c.guarded) || decorators.iter().any(|d| d.name == "UseGuards" || AUTH_MARKERS.contains(&d.name.as_str()));
        let prefix = class.and_then(|c| c.prefix.clone());
        for d in &decorators {
            if let (Some(prefix), Some((_, verb))) = (&prefix, NEST_VERBS.iter().find(|(n, _)| *n == d.name)) {
                let path = d.args.first().and_then(|a| string_of(src, *a)).unwrap_or_default();
                self.out.routes.push(RouteFact { handler: id.clone(), method: (*verb).into(), path: join(prefix, &path), framework: "nestjs".into(), span: src.span(node) });
            } else if matches!(d.name.as_str(), "MessagePattern" | "EventPattern" | "OnEvent" | "Cron" | "Interval" | "Timeout" | "Process" | "SubscribeMessage") {
                self.out.entry(&id, EntryKind::EventHandler, format!("event handler @{}", d.name));
            }
        }
        if guarded && prefix.is_some() {
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

        // Server routes: `app.get("/x", handler)`, `router.post("/x", auth, handler)`.
        let routerish = matches!(recv_last.as_str(), "app" | "router" | "server" | "fastify" | "routes" | "api") || recv_last.ends_with("router");
        if routerish
            && ROUTE_VERBS.contains(&name.as_str())
            && let Some(path) = first_string.as_ref().filter(|p| p.starts_with('/') || p.as_str() == "*")
        {
            let method = match name.as_str() {
                "del" => "DELETE".to_owned(),
                "all" => "ANY".to_owned(),
                other => other.to_uppercase(),
            };
            let handler = self.handler_of(args.last().copied()).unwrap_or_else(|| owner.clone());
            self.out.routes.push(RouteFact { handler, method, path: path.clone(), framework: self.server.into(), span: src.span(node) });
            return;
        }
        if routerish && name == "use" {
            // `app.use("/api", router)`: a mount; `app.use((err, req, res, next) => ...)`: an error handler.
            if let (Some(prefix), Some(target)) = (first_string.as_ref(), args.get(1)) {
                self.out.prefixes.push(RoutePrefix { kind: "mount".into(), target: src.norm(*target), prefix: prefix.clone(), framework: self.server.into(), span: src.span(node) });
            }
            if let Some(f) = args.last().filter(|a| crate::syntax::is_function_value(a.kind()))
                && parameters(src, *f).len() == 4
            {
                let handler = self.handler_of(Some(*f)).unwrap_or_else(|| owner.clone());
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
                    self.out.prefixes.push(RoutePrefix { kind: "global".into(), target: String::new(), prefix, framework: "nestjs".into(), span: src.span(node) });
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

    /// The symbol of a route handler argument: an inline function with a Blueprint, or a named function.
    fn handler_of(&self, arg: Option<Node>) -> Option<String> {
        let arg = arg?;
        if let Some(id) = self.function_ids.get(&arg.start_byte()) {
            return Some(id.clone());
        }
        if matches!(arg.kind(), "identifier" | "member_expression") {
            let text = self.src.norm(arg);
            let name = text.rsplit('.').next().unwrap_or(&text);
            return self
                .symbols
                .iter()
                .find(|s| s.name == name && matches!(s.kind, SymbolKind::Function | SymbolKind::Method))
                .map(|s| s.id.clone());
        }
        None
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
