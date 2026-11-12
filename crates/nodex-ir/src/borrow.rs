//! Symbolic borrow origins retained with lowered Rust files for on-demand lookup.

use crate::{BindingOrigin, NodeId, Span, semantic::ByteRange};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[ts(export)]
pub enum BorrowOriginKind {
    Local,
    Shared,
    Temporary,
    Static,
    Caller,
    Unknown,
}

/// A short origin shown beside a reference on the Blueprint.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq)]
#[ts(export)]
pub struct BorrowSource {
    pub kind: BorrowOriginKind,
    pub owner: Option<String>,
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub binding: Option<BindingOrigin>,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq)]
#[ts(export)]
pub struct BorrowPreview {
    pub node: NodeId,
    pub pin: String,
    pub sources: Vec<BorrowSource>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorrowBinding {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorrowRoot {
    Local(BorrowBinding),
    Parameter(usize),
    Shared(BorrowBinding),
    Temporary(Span),
    Static(Span),
    Call(Span),
    CallComponent { call: Span, index: usize },
    /// A parameter supplied by an inline callback API. The exact call must be resolved.
    CallbackInput { call: Span, argument: usize },
    /// A field that may require implicit Deref. Follow it only with a verified direct field.
    Field { site: Span, base: BorrowBinding, referent: Vec<BorrowPath> },
    /// A raw pointer value, which carries no borrow origin.
    RawPointer,
    /// A dereference whose operand type may be supplied by semantic analysis later.
    TypedPlace {
        binding: BorrowBinding,
        fallback: Vec<BorrowPath>,
    },
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorrowPath {
    pub root: BorrowRoot,
    /// Projections from the root place, in source spelling (for example ".name").
    pub projection: String,
    pub through: Vec<BorrowBinding>,
    pub mutable: bool,
}

impl BorrowPath {
    pub fn from(root: BorrowRoot) -> Self {
        Self {
            root,
            projection: String::new(),
            through: Vec::new(),
            mutable: false,
        }
    }

    pub fn unknown() -> Self {
        Self::from(BorrowRoot::Unknown)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorrowUse {
    pub span: Span,
    pub paths: Vec<BorrowPath>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorrowArgument {
    pub value: Vec<BorrowPath>,
    pub place: Vec<BorrowPath>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorrowCall {
    pub span: Span,
    pub receiver: Option<BorrowArgument>,
    pub arguments: Vec<BorrowArgument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub callbacks: Vec<BorrowCallback>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorrowCallback {
    pub argument: usize,
    pub returns: Vec<BorrowPath>,
}

/// Selects the receiver of a call (the first argument for a qualified method call).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorrowInput {
    pub argument: Option<usize>,
    pub value: bool,
}

/// A projection or callback relationship verified at one source occurrence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorrowFact {
    pub site: ByteRange,
    pub rule: BorrowRule,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorrowRule {
    Field { by_reference: bool },
    Lookup { input: BorrowInput, projection: String },
    Split { input: BorrowInput, at: String },
    Callback { input: BorrowInput, argument: usize },
    Payload { input: BorrowInput },
    Wrap,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorrowModel {
    pub parameters: Vec<Option<BorrowBinding>>,
    pub uses: Vec<BorrowUse>,
    pub returns: Vec<BorrowPath>,
    pub calls: Vec<BorrowCall>,
}
