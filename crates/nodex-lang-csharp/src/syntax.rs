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

pub fn named_children<'t>(node: Node<'t>) -> Vec<Node<'t>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
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
