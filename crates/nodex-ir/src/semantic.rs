//! Semantic facts: what a compiler-grade analyzer knows that syntax alone does not.
//!
//! Semantic providers (the Roslyn sidecar for C#, later the TypeScript API and rust-analyzer)
//! report facts keyed by **byte ranges** of the files on disk. `nodex-core` joins them with the
//! syntax-only IR through those ranges, so providers never need to know Nodex symbol IDs.
//! This file is also the JSON contract the sidecars emit.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Language;

/// A byte range in a project file (`file` is project-relative with `/`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ByteRange {
    pub file: String,
    pub start: u32,
    pub end: u32,
}

impl ByteRange {
    pub fn contains(&self, other: &ByteRange) -> bool {
        self.file == other.file && self.start <= other.start && other.end <= self.end
    }
}

/// What kind of reference a [`SemanticRef`] is.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    /// A method call; `site` is the whole invocation expression.
    Call,
    /// An object creation; `site` is the creation expression.
    Construct,
    /// A base-list entry; `site` is the base type in the deriving type's declaration.
    Base,
    /// `event += handler`; `site` is the assignment expression; the target is the event.
    Subscribe,
    /// `event -= handler`.
    Unsubscribe,
    /// The method group on the right side of a subscription; same `site` as the subscription.
    Handler,
    /// `Event?.Invoke(...)` or `Event(...)`; `site` is the invocation; the target is the event.
    Raise,
    /// A type that has every member of an interface without declaring it (TypeScript structural
    /// typing); `site` is the type's name; the target is the interface.
    Compatible,
}

/// The symbol a reference resolves to.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SemanticTarget {
    /// Declaration range, for targets declared in the project.
    pub decl: Option<ByteRange>,
    /// Human-readable name (`OrderService.ShipAsync(int)`).
    pub display: String,
    /// Fully qualified containing type, if any (`System.Console`).
    pub container: Option<String>,
    /// `method`, `constructor`, `type`, `event`, `property`, `unresolved`, ...
    pub kind: String,
    /// True for standard LINQ operators (`System.Linq.Enumerable` / `Queryable`).
    #[serde(default)]
    pub linq: bool,
    pub external: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EmitFocus {
    /// True when the receiver represents the event source. Otherwise `text` is the event argument.
    pub receiver: bool,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SemanticRef {
    pub site: ByteRange,
    pub kind: RefKind,
    pub target: SemanticTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emit_focus: Option<EmitFocus>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NavigationFact {
    pub site: ByteRange,
    pub target: SemanticTarget,
    #[serde(default)]
    pub implementation: bool,
    /// Kind of the implementing declaration when it differs from the contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_kind: Option<String>,
}

/// The inferred type of a local variable (`var x = ...`, `foreach (var x in ...)`, `out var x`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LocalTypeFact {
    /// Range of the variable's identifier at its declaration.
    pub site: ByteRange,
    pub name: String,
    pub ty: String,
}

/// A use of a binding resolved by the compiler, including uses outside a lowered flow.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BindingUseFact {
    pub site: ByteRange,
    pub declaration: ByteRange,
}

/// Nominal type identity for a Rust binding or constructor expression.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RustNominalType {
    pub site: ByteRange,
    pub declaration: ByteRange,
    pub by_reference: bool,
}

/// An explanation verified by the compiler for an expression at this byte range.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ExpressionFact {
    pub site: ByteRange,
    pub kind: ExpressionFactKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpressionFactKind {
    Narrowed,
    Boxing,
    ValueCopy,
    ReferenceCopy,
    Dynamic,
    Freeze,
    Spawn,
    Sleep,
    ChannelSend,
    ProcessExit,
}

/// An exact source site classified by a language's type checker.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ValueFact {
    pub site: ByteRange,
    pub target: ValueFactTarget,
    pub info: crate::ValueInfo,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TypeFact {
    pub site: ByteRange,
    pub target: ValueFactTarget,
    pub info: crate::types::TypeInfo,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ValueFactTarget {
    Binding,
    Expression,
    Member,
}

/// Everything a semantic provider reports for a project.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SemanticFacts {
    /// What produced the facts (`Roslyn · MSBuild · Sample.csproj`).
    pub source: String,
    pub refs: Vec<SemanticRef>,
    pub locals: Vec<LocalTypeFact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expressions: Vec<ExpressionFact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<ValueFact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<TypeFact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub binding_uses: Vec<BindingUseFact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rust_nominal_types: Vec<RustNominalType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reading: Vec<crate::reading::ReadingFact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rust_borrows: Vec<crate::borrow::BorrowFact>,
    /// Present when the provider collects project-wide symbol navigation facts.
    #[serde(default)]
    pub navigation: Option<Vec<NavigationFact>>,
    /// Declaration names of type aliases the compiler recognizes as object contracts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub implementation_contracts: Vec<ByteRange>,
    /// Project-relative configuration dependencies read by the provider.
    #[serde(default)]
    pub configuration_files: Vec<String>,
    /// Problems worth showing (failed project loads, unresolved references count, ...).
    #[serde(default)]
    pub messages: Vec<String>,
}

/// Progress of semantic analysis for one language, shown in the UI.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum SemanticState {
    Loading,
    Ready,
    Unavailable,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct SemanticStatus {
    pub language: Language,
    pub state: SemanticState,
    /// Provider description when ready; the reason when unavailable.
    pub detail: String,
    pub messages: Vec<String>,
}
