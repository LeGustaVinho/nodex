//! Helpers over tree-sitter syntax trees shared by the language frontends.

use nodex_ir::{Focus, Span};
use tree_sitter::Node;

use crate::kit::{LineIndex, normalize_ws};

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

    /// Keep the whole operand and preserve whitespace within literals.
    pub fn focus(&self, node: Node) -> Option<Focus> {
        self.focus_range(node.start_byte(), node.end_byte())
    }

    pub fn focus_text(text: &str) -> Option<Focus> {
        let text = text.trim();
        (!text.is_empty()).then(|| Focus { primary: text.to_owned(), prefix: None, prefix_bytes: None, primary_bytes: None })
    }

    pub fn focus_range(&self, start: usize, end: usize) -> Option<Focus> {
        let text = self.text.get(start..end)?;
        let mut focus = Self::focus_text(text)?;
        let start = start + text.len() - text.trim_start().len();
        focus.primary_bytes = Some([start as u32, (start + focus.primary.len()) as u32]);
        Some(focus)
    }

    /// Keep the receiver and punctuation together so the member can be emphasized separately.
    pub fn member_focus(&self, access: Node, member: Node) -> Option<Focus> {
        self.split_focus(access.start_byte(), member.start_byte(), member.end_byte())
    }

    /// Splits a qualified target while keeping the entire source suffix in the primary text.
    pub fn split_focus(&self, start: usize, primary: usize, end: usize) -> Option<Focus> {
        let mut focus = self.focus_range(primary, end)?;
        let raw = self.text.get(start..primary)?;
        let prefix = raw.trim_start();
        const PREVIEW_BYTES: usize = 96;
        if prefix.len() > PREVIEW_BYTES {
            let limit = prefix.floor_char_boundary(PREVIEW_BYTES);
            focus.prefix = Some(format!("{}…", &prefix[..limit]));
            focus.prefix_bytes = Some([(primary - prefix.len()) as u32, primary as u32]);
        } else if !prefix.is_empty() {
            focus.prefix = Some(prefix.to_owned());
        }
        Some(focus)
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

