//! Symbolic borrow origins retained with lowered Rust files for on-demand lookup.

use crate::Span;
use serde::{Deserialize, Serialize};

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
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorrowModel {
    pub parameters: Vec<Option<BorrowBinding>>,
    pub uses: Vec<BorrowUse>,
    pub returns: Vec<BorrowPath>,
    pub calls: Vec<BorrowCall>,
}
