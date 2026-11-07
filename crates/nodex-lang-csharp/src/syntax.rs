//! Small helpers over tree-sitter nodes.

use nodex_ir::Span;
use nodex_lang_api::kit::{LineIndex, normalize_ws};
use tree_sitter::Node;

/// The source being lowered, passed around by copy.
#[derive(Clone, Copy)]
pub struct Src<'a> {
    pub text: &'a str,
    pub file: &'a str,
    pub index: &'a LineIndex<'a>,
}

impl<'a> Src<'a> {
    pub fn text(&self, node: Node) -> &'a str {
        &self.text[node.byte_range()]
    }

    /// Node text with whitespace collapsed.
    pub fn norm(&self, node: Node) -> String {
        normalize_ws(self.text(node))
    }

    pub fn span(&self, node: Node) -> Span {
        self.index.span(self.file, node.start_byte(), node.end_byte())
    }

    /// Text from the start of `node` up to (not including) `until`, normalized. Used for
    /// statement headers such as `foreach (var x in xs)`.
    pub fn header(&self, node: Node, until: Option<Node>) -> String {
        let end = until.map(|u| u.start_byte()).unwrap_or(node.end_byte());
        normalize_ws(&self.text[node.start_byte()..end.max(node.start_byte())])
    }

    /// Span from the start of `node` to the start of `until` (or the end of `node`).
    pub fn header_span(&self, node: Node, until: Option<Node>) -> Span {
        let end = until.map(|u| u.start_byte()).unwrap_or(node.end_byte());
        self.index.span(self.file, node.start_byte(), end.max(node.start_byte()))
    }
}

/// Named children, without comments (comments are "extras" that may appear anywhere).
pub fn named_children<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).filter(|c| c.kind() != "comment").collect()
}

pub fn children<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    let mut cursor = node.walk();
    node.children(&mut cursor).collect()
}

/// Named children of `kind`.
pub fn children_of_kind<'t>(node: Node<'t>, kind: &str) -> Vec<Node<'t>> {
    named_children(node)
        .into_iter()
        .filter(|c| c.kind() == kind)
        .collect()
}

pub fn first_child_of_kind<'t>(node: Node<'t>, kind: &str) -> Option<Node<'t>> {
    named_children(node).into_iter().find(|c| c.kind() == kind)
}

/// Depth-first search for the first `ERROR` or missing node.
pub fn first_error(node: Node) -> Option<Node> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }
    for child in children(node) {
        if child.has_error()
            && let Some(found) = first_error(child) {
                return Some(found);
            }
    }
    None
}

/// True if any descendant of `node` (not crossing nested functions or lambdas) has `kind`.
pub fn contains_kind(node: Node, kind: &str) -> bool {
    for child in named_children(node) {
        if child.kind() == kind {
            return true;
        }
        if matches!(
            child.kind(),
            "lambda_expression" | "anonymous_method_expression" | "local_function_statement"
        ) {
            continue;
        }
        if contains_kind(child, kind) {
            return true;
        }
    }
    false
}

/// Modifier keywords (`public`, `static`, `async`, ...) of a declaration.
pub fn modifiers(src: Src, decl: Node) -> Vec<String> {
    children_of_kind(decl, "modifier")
        .into_iter()
        .map(|m| src.norm(m))
        .collect()
}

/// Attribute names of a declaration, as `attr:Name` flavors.
pub fn attributes(src: Src, decl: Node) -> Vec<String> {
    let mut out = Vec::new();
    for list in children_of_kind(decl, "attribute_list") {
        for attr in children_of_kind(list, "attribute") {
            if let Some(name) = attr.child_by_field_name("name") {
                out.push(format!("attr:{}", src.norm(name)));
            }
        }
    }
    out
}

/// The `///` documentation comment right before a declaration, as plain text: the `<summary>`
/// when present, XML tags stripped (`<see cref="X"/>` becomes `X`).
pub fn doc_comment(src: Src, node: Node) -> Option<String> {
    let mut lines = Vec::new();
    let mut cur = node.prev_sibling();
    while let Some(c) = cur {
        if c.kind() != "comment" {
            break;
        }
        let t = src.text(c).trim();
        if !t.starts_with("///") {
            break;
        }
        lines.push(t.trim_start_matches('/').trim().to_owned());
        cur = c.prev_sibling();
    }
    if lines.is_empty() {
        return None;
    }
    lines.reverse();
    let xml = lines.join(" ");
    let section = |tag: &str| -> Option<String> {
        let start = xml.find(&format!("<{tag}"))?;
        let open_end = xml[start..].find('>')? + start + 1;
        let close = xml[open_end..].find(&format!("</{tag}>"))? + open_end;
        Some(xml[open_end..close].to_owned())
    };
    let mut text = section("summary").unwrap_or_else(|| xml.clone());
    if let Some(r) = section("returns") {
        text.push_str(&format!(" Returns: {r}"));
    }
    // `<see cref="Foo.Bar"/>` → `Foo.Bar`; other tags vanish.
    let mut out = String::new();
    let mut rest = text.as_str();
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let Some(gt) = rest[lt..].find('>') else {
            rest = &rest[lt..];
            break;
        };
        let tag = &rest[lt..lt + gt + 1];
        for attr in ["cref=\"", "name=\"", "langword=\""] {
            if let Some(a) = tag.find(attr) {
                let v = &tag[a + attr.len()..];
                if let Some(end) = v.find('"') {
                    let value = &v[..end];
                    out.push_str(value.rsplit(':').next().unwrap_or(value));
                }
                break;
            }
        }
        rest = &rest[lt + gt + 1..];
    }
    out.push_str(rest);
    let clean = out.split_whitespace().collect::<Vec<_>>().join(" ");
    (!clean.is_empty()).then_some(clean)
}

/// Directives that do not hold code (regions, pragmas, defines, ...).
pub fn is_inert_directive(kind: &str) -> bool {
    kind.starts_with("preproc_") && !matches!(kind, "preproc_if" | "preproc_elif" | "preproc_else")
}

/// `#if DEBUG`, `#elif TRACE` or `#else` for a conditional-compilation node.
pub fn preproc_label(src: Src, node: Node) -> String {
    match node.kind() {
        "preproc_else" => "#else".to_owned(),
        kind => {
            let keyword = if kind == "preproc_elif" { "#elif" } else { "#if" };
            let cond = node.child_by_field_name("condition").map(|c| src.norm(c)).unwrap_or_default();
            format!("{keyword} {cond}")
        }
    }
}

/// The code nodes directly inside a conditional-compilation branch (not its condition or the
/// next `#elif`/`#else`).
pub fn preproc_body<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    let skip: Vec<usize> = ["condition", "alternative"]
        .iter()
        .filter_map(|f| node.child_by_field_name(f))
        .map(|n| n.id())
        .collect();
    named_children(node).into_iter().filter(|c| !skip.contains(&c.id())).collect()
}

/// Flattens `#if`/`#elif`/`#else` around declarations: every code node with the build
/// conditions it depends on (`["#if DEBUG"]`). Regions and pragmas are dropped.
pub fn flatten_preproc<'t>(src: Src, nodes: Vec<Node<'t>>, conds: &[String], out: &mut Vec<(Node<'t>, Vec<String>)>) {
    for n in nodes {
        match n.kind() {
            "preproc_if" | "preproc_elif" | "preproc_else" => {
                let mut inner = conds.to_vec();
                inner.push(preproc_label(src, n));
                flatten_preproc(src, preproc_body(n), &inner, out);
                if let Some(alt) = n.child_by_field_name("alternative") {
                    flatten_preproc(src, vec![alt], conds, out);
                }
            }
            kind if is_inert_directive(kind) => {}
            _ => out.push((n, conds.to_vec())),
        }
    }
}

/// Parameters of a declaration: `(name, type, modifiers)`. Handles `params T[] name`, which the
/// grammar attaches to the parameter list itself.
pub fn parameters(src: Src, node: Node) -> Vec<(String, Option<String>, Vec<String>)> {
    let Some(list) = node.child_by_field_name("parameters").or_else(|| first_child_of_kind(node, "parameter_list")) else {
        return Vec::new();
    };
    let mut out: Vec<(String, Option<String>, Vec<String>)> = children_of_kind(list, "parameter")
        .into_iter()
        .map(|p| {
            let name = p.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_default();
            let ty = p.child_by_field_name("type").map(|t| src.norm(t));
            let mods = children(p)
                .into_iter()
                .filter(|c| c.kind() == "modifier" || c.kind() == "parameter_modifier")
                .map(|c| src.norm(c))
                .collect();
            (name, ty, mods)
        })
        .collect();
    if let (Some(name), Some(ty)) = (list.child_by_field_name("name"), list.child_by_field_name("type")) {
        out.push((src.norm(name), Some(src.norm(ty)), vec!["params".to_owned()]));
    }
    out
}

/// Parameter types for symbol ids: `in int`, `ref string`, `params int[]`.
pub fn id_types(params: &[(String, Option<String>, Vec<String>)]) -> Vec<String> {
    params
        .iter()
        .map(|(_, t, mods)| {
            let kind: Vec<&str> = mods.iter().map(String::as_str).filter(|m| matches!(*m, "in" | "ref" | "out" | "params")).collect();
            let ty = t.clone().unwrap_or_else(|| "?".into());
            if kind.is_empty() { ty } else { format!("{} {ty}", kind.join(" ")) }
        })
        .collect()
}

/// Id of a local function: `<owner method id>.<name>(<types>)`. Shared by structure and flow
/// lowering so the call target on the declaring node matches the function's own Blueprint.
pub fn local_function_id(src: Src, owner: &str, node: Node) -> String {
    let name = node.child_by_field_name("name").map(|n| src.norm(n)).unwrap_or_else(|| "?".into());
    format!("{owner}.{name}({})", id_types(&parameters(src, node)).join(", "))
}

/// Strips generic arguments and whitespace: `List<Order>` → `List`, `global::A.B` → `A.B`.
pub fn bare_type_name(text: &str) -> String {
    let mut depth = 0;
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth -= 1,
            '?' | '[' | ']' | ' ' if depth == 0 => {}
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out.trim_start_matches("global::").to_owned()
}

/// Every type name mentioned in a type expression, including generic arguments:
/// `Dictionary<int, Order>` → [`Dictionary`, `Order`]. Predefined types are skipped.
pub fn type_names(src: Src, ty: Node) -> Vec<String> {
    let mut out = Vec::new();
    collect_type_names(src, ty, &mut out);
    out
}

fn collect_type_names(src: Src, node: Node, out: &mut Vec<String>) {
    match node.kind() {
        "identifier" => out.push(src.norm(node)),
        "qualified_name" => out.push(bare_type_name(src.text(node))),
        "generic_name" => {
            if let Some(id) = first_child_of_kind(node, "identifier") {
                out.push(src.norm(id));
            }
            if let Some(args) = first_child_of_kind(node, "type_argument_list") {
                for arg in named_children(args) {
                    collect_type_names(src, arg, out);
                }
            }
        }
        "predefined_type" | "implicit_type" => {}
        _ => {
            for child in named_children(node) {
                collect_type_names(src, child, out);
            }
        }
    }
}
