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
pub struct SemanticRef {
    pub site: ByteRange,
    pub kind: RefKind,
    pub target: SemanticTarget,
}

/// The inferred type of a local variable (`var x = ...`, `foreach (var x in ...)`, `out var x`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LocalTypeFact {
    /// Range of the variable's identifier at its declaration.
    pub site: ByteRange,
    pub name: String,
    pub ty: String,
}

/// Everything a semantic provider reports for a project.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SemanticFacts {
    /// What produced the facts (`Roslyn · MSBuild · Sample.csproj`).
    pub source: String,
    pub refs: Vec<SemanticRef>,
    pub locals: Vec<LocalTypeFact>,
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
