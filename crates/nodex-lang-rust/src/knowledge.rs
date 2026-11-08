//! Knowledge facts of a Rust file: axum, actix-web and Rocket routes, reqwest requests,
//! environment reads, diesel and SeaORM entities and writes, tests, and entry points (`main`,
//! Tauri commands, wasm and FFI exports, server start).
//!
//! The detection rules were ported from CodeGraph (`crates/core/src/extract/rust.rs`). This pass
//! walks the same syntax tree as the structure and flow passes, after them.

use nodex_ir::knowledge::{ConfigFact, EntityFact, EntryKind, FileKnowledge, HttpCallFact, MutationFact, RouteFact};
use nodex_ir::{Symbol, SymbolKind};
use nodex_lang_api::knowledge::{Owners, call_capabilities, unquote};
use tree_sitter::Node;

use crate::syntax::{Src, attributes, named_children};

const VERBS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options"];

struct Walker<'a> {
    src: Src<'a>,
    owners: &'a Owners,
    symbols: &'a [Symbol],
    /// `actix` or `rocket`: who reads `#[get("/x")]` attributes in this file.
    attr_framework: &'static str,
    out: FileKnowledge,
}

pub fn extract(src: Src, root: Node, symbols: &[Symbol]) -> FileKnowledge {
    let owners = Owners::new(symbols, src.file);
    let attr_framework = if src.text.contains("rocket") { "rocket" } else { "actix" };
    let mut w = Walker { src, owners: &owners, symbols, attr_framework, out: FileKnowledge::default() };
    w.walk(root);
    w.out
}

impl Walker<'_> {
    fn walk(&mut self, node: Node) {
        match node.kind() {
            "function_item" => self.function(node),
            "struct_item" => self.entity(node),
            "call_expression" => self.call(node),
            "macro_invocation" => self.macro_call(node),
            _ => {}
        }
        for child in named_children(node) {
            self.walk(child);
        }
    }

    fn function(&mut self, node: Node) {
        let src = self.src;
        let Some(id) = self
            .owners
            .declared_at(node.start_byte(), node.end_byte(), |k| matches!(k, SymbolKind::Function | SymbolKind::Method))
            .or_else(|| node.child_by_field_name("body").and_then(|b| self.owners.function_at(b.start_byte())))
            .map(str::to_owned)
        else {
            return;
        };
        let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
        let parent_is_root = self.symbols.iter().find(|s| s.id == id).and_then(|s| s.parent.as_deref()).is_some_and(|p| !p.contains("::"));
        if name == "main" && parent_is_root {
            self.out.entry(&id, EntryKind::Main, "program entry (main)");
        }
        for (path, args) in attributes(src, node) {
            let last = path.rsplit("::").next().unwrap_or(&path);
            match last {
                "test" | "rstest" | "test_case" => {
                    self.out.symbol_mut(&id).test = true;
                    self.out.entry(&id, EntryKind::Test, format!("test #[{path}]"));
                }
                "command" if path.contains("tauri") => self.out.entry(&id, EntryKind::Export, "Tauri command"),
                "wasm_bindgen" => self.out.entry(&id, EntryKind::Export, "wasm export"),
                "no_mangle" => self.out.entry(&id, EntryKind::Export, "FFI export"),
                "pyfunction" => self.out.entry(&id, EntryKind::Export, "Python binding"),
                "napi" => self.out.entry(&id, EntryKind::Export, "Node binding"),
                verb if VERBS.contains(&verb) || verb == "route" => {
                    let inner = args.trim_matches(['(', ')']);
                    let Some(path) = inner.split(',').next().and_then(unquote).filter(|p| p.starts_with('/')) else { continue };
                    let method = if verb == "route" { "ANY".to_owned() } else { verb.to_uppercase() };
                    self.out.routes.push(RouteFact {
                        handler: id.clone(),
                        handler_ref: None,
                        method,
                        path,
                        framework: self.attr_framework.into(),
                        span: src.span(node),
                    });
                }
                _ => {}
            }
        }
    }

    /// `#[derive(Queryable)] #[diesel(table_name = invoices)] struct Invoice`, SeaORM `DeriveEntityModel`.
    fn entity(&mut self, node: Node) {
        let src = self.src;
        let attrs = attributes(src, node);
        let derives: Vec<String> = attrs.iter().filter(|(p, _)| p == "derive").flat_map(|(_, a)| a.trim_matches(['(', ')']).split(',').map(|d| d.trim().rsplit("::").next().unwrap_or("").to_owned()).collect::<Vec<_>>()).collect();
        let framework = if derives.iter().any(|d| matches!(d.as_str(), "Queryable" | "Insertable" | "Selectable" | "Identifiable" | "AsChangeset")) {
            "diesel"
        } else if derives.iter().any(|d| d == "DeriveEntityModel") {
            "sea-orm"
        } else {
            return;
        };
        let Some(id) = self.owners.declared_at(node.start_byte(), node.end_byte(), SymbolKind::is_type).map(str::to_owned) else { return };
        let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
        let table = attrs.iter().find(|(p, _)| p == "diesel" || p == "sea_orm").and_then(|(_, a)| {
            let at = a.find("table_name")?;
            let rest = a[at + "table_name".len()..].trim_start().trim_start_matches('=').trim_start();
            let value: String = rest.chars().take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '"')).collect();
            Some(value.trim_matches('"').to_owned())
        });
        self.out.entities.push(EntityFact { owner: id, entity: name, property: table.clone(), framework: framework.into(), table, span: src.span(node) });
    }

    fn call(&mut self, node: Node) {
        let src = self.src;
        let Some(function) = node.child_by_field_name("function") else { return };
        let args: Vec<Node> = node.child_by_field_name("arguments").map(named_children).unwrap_or_default();
        let (receiver, name) = match function.kind() {
            "field_expression" => (
                function.child_by_field_name("value").map(|v| src.norm(v)).unwrap_or_default(),
                function.child_by_field_name("field").map(|f| src.norm(f)).unwrap_or_default(),
            ),
            "scoped_identifier" => {
                let text = src.norm(function);
                let (head, last) = text.rsplit_once("::").map(|(h, l)| (h.to_owned(), l.to_owned())).unwrap_or((String::new(), text.clone()));
                (head, last)
            }
            "generic_function" => (String::new(), function.child_by_field_name("function").map(|f| src.norm(f)).unwrap_or_default()),
            _ => (String::new(), src.norm(function)),
        };
        let Some(owner) = self.owners.function_at(node.start_byte()).map(str::to_owned) else { return };
        for tag in call_capabilities(&name, &receiver) {
            self.out.capability(&owner, tag);
        }
        let recv = receiver.to_ascii_lowercase();
        let first_string = args.first().and_then(|a| unquote(src.text(*a)));
        match (recv.rsplit("::").next().unwrap_or(&recv), name.as_str()) {
            // axum / actix builders: `.route("/x", get(list).post(create))`, `.route("/x", web::get().to(h))`.
            (_, "route") => {
                if let (Some(path), Some(methods)) = (first_string.filter(|p| p.starts_with('/')), args.get(1)) {
                    let framework = if src.norm(*methods).contains("web::") || src.text.contains("actix") { "actix" } else { "axum" };
                    for (verb, written) in method_handlers(src, *methods) {
                        let handler = self.function_named(&written).unwrap_or_else(|| owner.clone());
                        // `handlers::list` may live in another module: `nodex-core` resolves the path.
                        let is_path = !written.is_empty() && written.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | ':'));
                        let handler_ref = is_path.then_some(written);
                        self.out.routes.push(RouteFact { handler, handler_ref, method: verb, path: path.clone(), framework: framework.into(), span: src.span(node) });
                    }
                }
            }
            ("env" | "std::env" | "dotenvy" | "dotenv", "var" | "var_os") => {
                if let Some(key) = first_string {
                    self.out.config.push(ConfigFact { owner, key, source: "env".into(), default: None, span: src.span(node) });
                }
            }
            ("reqwest", verb) if VERBS.contains(&verb) => self.http(&owner, verb, args.first().copied(), "reqwest", node),
            (r, verb) if (r.ends_with("client") || recv.ends_with("client")) && VERBS.contains(&verb) => {
                self.http(&owner, verb, args.first().copied(), "reqwest", node)
            }
            ("diesel", "insert_into" | "update" | "delete" | "replace_into") => self.diesel_write(&owner, args.first().copied(), node),
            // `use diesel::prelude::*` brings the DSL in scope: `insert_into(invoices::table)`,
            // `update(invoices.find(id))`. Bare `update`/`delete` count only on a table expression.
            ("", "insert_into" | "replace_into" | "update" | "delete") if src.text.contains("diesel") => {
                let target = args.first().map(|a| src.norm(*a)).unwrap_or_default();
                let table_like = target.contains("::table") || target.contains(".find(") || target.contains(".filter(");
                if matches!(name.as_str(), "insert_into" | "replace_into") || table_like {
                    self.diesel_write(&owner, args.first().copied(), node);
                }
            }
            ("axum", "serve") | ("warp", "serve") | ("httpserver", "new") | ("rocket", "build") | ("server", "bind") => {
                self.out.entry(&owner, EntryKind::ServerStart, "server start");
            }
            _ => {}
        }
    }

    /// `env!("X")`, `option_env!("X")`.
    fn macro_call(&mut self, node: Node) {
        let src = self.src;
        let Some(name) = node.child_by_field_name("macro").map(|m| src.norm(m)) else { return };
        if name != "env" && name != "option_env" {
            return;
        }
        let tokens = named_children(node).into_iter().find(|c| c.kind() == "token_tree").map(|t| src.norm(t)).unwrap_or_default();
        let Some(key) = unquote(tokens.trim_matches(['(', ')']).split(',').next().unwrap_or("")) else { return };
        let Some(owner) = self.owners.function_at(node.start_byte()).map(str::to_owned) else { return };
        self.out.config.push(ConfigFact { owner, key, source: "env".into(), default: None, span: src.span(node) });
    }

    fn http(&mut self, owner: &str, verb: &str, url: Option<Node>, client: &str, node: Node) {
        let src = self.src;
        let url = url.map(|u| unquote(src.text(u)).unwrap_or_else(|| format!("${{{}}}", src.norm(u)))).unwrap_or_default();
        self.out.http_calls.push(HttpCallFact { owner: owner.to_owned(), method: verb.to_uppercase(), url, client: client.into(), span: src.span(node) });
    }

    /// A diesel write to the table named by the first argument (`invoices::table`, `invoices.find(id)`).
    fn diesel_write(&mut self, owner: &str, table: Option<Node>, node: Node) {
        let text = table.map(|a| self.src.norm(a)).unwrap_or_default();
        let table = text.split([':', '.', '(']).next().unwrap_or(&text).trim().to_owned();
        self.out.mutations.push(MutationFact { owner: owner.to_owned(), target: table, kind: "database".into(), span: self.src.span(node) });
    }

    fn function_named(&self, name: &str) -> Option<String> {
        let name = name.rsplit("::").next().unwrap_or(name);
        self.symbols.iter().find(|s| s.name == name && matches!(s.kind, SymbolKind::Function | SymbolKind::Method)).map(|s| s.id.clone())
    }
}

/// `get(list).post(create)` → `[("GET", "list"), ("POST", "create")]`; `web::get().to(h)` → `[("GET", "h")]`.
fn method_handlers(src: Src, node: Node) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let text = src.norm(node);
    // actix: `web::get().to(handler)`.
    if let Some(to) = text.rfind(".to(") {
        let handler = text[to + 4..].trim_end_matches(')').trim().to_owned();
        if let Some(verb) = VERBS.iter().find(|v| text.contains(&format!("{v}()"))) {
            out.push((verb.to_uppercase(), handler));
        }
        return out;
    }
    // axum: `get(a).post(b)`, `routing::get(a)`.
    let mut rest = text.as_str();
    while let Some(open) = rest.find('(') {
        let head = &rest[..open];
        let verb = head.rsplit(['.', ':', ' ']).next().unwrap_or("");
        let close = rest[open..].find(')').map(|c| open + c).unwrap_or(rest.len());
        if VERBS.contains(&verb) {
            out.push((verb.to_uppercase(), rest[open + 1..close].trim().to_owned()));
        }
        rest = &rest[(close + 1).min(rest.len())..];
    }
    out
}
