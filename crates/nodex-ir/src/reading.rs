//! Source-linked explanations that can be rendered independently of execution edges.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Access, Concept, Expr, FlowGraph, NodeId, Span, ValueBasis, types::TypeInfo};

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct ReadingSite {
    pub node: NodeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub path: Option<String>,
    pub span: Span,
    pub basis: ValueBasis,
    pub content: ReadingContent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub borrow_sources: Option<Vec<crate::borrow::BorrowSource>>,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[serde(tag = "kind")]
#[ts(export)]
pub enum ReadingContent {
    Steps { label: String, steps: Vec<ReadingStep> },
    Paths {
        input: String,
        deferred: bool,
        branches: Vec<ReadingBranch>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        result: Option<TypeInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        timing: Option<String>,
    },
    Pattern {
        input: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        input_type: Option<TypeInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        source: Option<Span>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        route: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        route_label: Option<String>,
        parts: Vec<ReadingPart>,
    },
    Detail {
        label: String,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        definition: Option<Span>,
        #[serde(default)]
        borrow: bool,
    },
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct ReadingStep {
    pub span: Span,
    pub timing: String,
    pub action: String,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct ContractItem {
    pub group: String,
    pub text: String,
    pub span: Span,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub type_info: Option<TypeInfo>,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq)]
#[ts(export)]
pub struct ReadingBranch {
    pub label: String,
    pub action: String,
    pub output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pin: Option<String>,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct ReadingPart {
    pub projection: String,
    pub span: Span,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub binding: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub access: Option<Access>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub type_info: Option<TypeInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub borrow_sources: Option<Vec<crate::borrow::BorrowSource>>,
}

/// A frontend fact before it is joined to a particular Blueprint.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ReadingFact {
    pub anchor: Span,
    pub span: Span,
    pub basis: ValueBasis,
    pub content: ReadingContent,
}

fn contains(outer: &Span, inner: &Span) -> bool {
    outer.file == inner.file
        && outer.start_byte <= inner.start_byte
        && inner.end_byte <= outer.end_byte
}

fn in_separate_body(expr: &Expr, anchor: &Span) -> bool {
    contains(&expr.span, anchor)
        && (expr.concept == Concept::Lambda
            && expr.symbol.is_some()
            && expr.children.is_empty()
            && expr.span != *anchor
            || expr.children.iter().any(|child| in_separate_body(child, anchor)))
}

fn expression_anchor(expr: &Expr, anchor: &Span, path: String) -> Option<(u32, String)> {
    if !contains(&expr.span, anchor) {
        return None;
    }
    expr.children
        .iter()
        .enumerate()
        .filter_map(|(i, child)| expression_anchor(child, anchor, format!("{path}.{i}")))
        .min_by_key(|(size, _)| *size)
        .or_else(|| Some((expr.span.end_byte - expr.span.start_byte, path)))
}

impl FlowGraph {
    pub fn attach_reading<'a>(&mut self, facts: impl IntoIterator<Item = &'a ReadingFact>) {
        for fact in facts {
            if !contains(&self.span, &fact.anchor) {
                continue;
            }
            if self.nodes.iter().filter_map(|node| node.expr.as_ref()).any(|expr| in_separate_body(expr, &fact.anchor)) {
                continue;
            }
            let Some((_, node, path)) = self
                .nodes
                .iter()
                .filter_map(|node| {
                    if let Some((size, path)) = node
                        .expr
                        .as_ref()
                        .and_then(|expr| expression_anchor(expr, &fact.anchor, "0".into()))
                    {
                        // Prefer the expression to its enclosing statement on an equal span.
                        let exact = size == fact.anchor.end_byte - fact.anchor.start_byte;
                        return Some((if exact { 0 } else { size + 2 }, node, Some(path)));
                    }
                    if node.span == fact.anchor {
                        return Some((1, node, None));
                    }
                    contains(&node.span, &fact.anchor)
                        .then(|| (node.span.end_byte - node.span.start_byte + 1, node, None))
                })
                .min_by_key(|(size, _, _)| *size)
            else {
                continue;
            };
            let entries = self.reading.get_or_insert_with(Vec::new);
            let site = ReadingSite {
                node: node.id.clone(),
                path,
                span: fact.span.clone(),
                basis: fact.basis,
                content: fact.content.clone(),
                borrow_sources: None,
            };
            // Resolved facts refine the syntax entry for the same occurrence.
            if let Some(previous) = entries.iter_mut().find(|previous| {
                previous.node == site.node
                    && previous.span == site.span
                    && std::mem::discriminant(&previous.content)
                        == std::mem::discriminant(&site.content)
            }) {
                *previous = site;
            } else {
                entries.push(site);
            }
        }
    }
}
