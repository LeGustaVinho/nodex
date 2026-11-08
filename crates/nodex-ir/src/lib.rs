//! # Nodex IR
//!
//! The language-agnostic intermediate representation shared by every Nodex component.
//!
//! The IR has two levels:
//! - **Structure graph** ([`StructureGraph`]): namespaces, types, members and the relations between them.
//! - **Flow graph** ([`FlowGraph`]): a Blueprint-style graph of one function body, built from universal
//!   [`Concept`]s so that the same logic looks the same in every language.
//!
//! Language frontends produce a [`FileIr`] per source file; `nodex-core` links them into a project.
//! TypeScript bindings are generated with `ts-rs` into `web/src/gen` by running `cargo test -p nodex-ir`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Identifier of a symbol in the structure graph (for example `Sample.Services.OrderService.Find(int)`).
pub type SymbolId = String;
/// Identifier of a node inside a flow graph. Stable across unrelated edits (see [`stable_id`]).
pub type NodeId = String;

/// A source language supported by a frontend.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Language {
    Csharp,
    Typescript,
    Rust,
}

/// A range of source text. Lines and columns are 1-based; columns count UTF-16 code units
/// (the unit editors such as Monaco use). Bytes are 0-based offsets into the UTF-8 file.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash, Default)]
#[ts(export)]
pub struct Span {
    /// Project-relative path with forward slashes.
    pub file: String,
    pub start_byte: u32,
    pub end_byte: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl Span {
    pub fn contains_byte(&self, byte: u32) -> bool {
        self.start_byte <= byte && byte < self.end_byte
    }
}

// ---------------------------------------------------------------------------------------------
// Structure graph
// ---------------------------------------------------------------------------------------------

/// What a symbol in the structure graph represents.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub enum SymbolKind {
    /// External dependency (NuGet package, npm package, crate).
    Package,
    /// File-level module (TypeScript/Rust modules).
    Module,
    Namespace,
    Class,
    Struct,
    Interface,
    Trait,
    Enum,
    /// Tagged union / discriminated union / Rust enum with data.
    Union,
    Record,
    Delegate,
    /// A named type expression (TypeScript `type X = ...`); exists only at compile time.
    TypeAlias,
    Function,
    Method,
    Constructor,
    Property,
    Field,
    Event,
    EnumMember,
}

impl SymbolKind {
    /// True for kinds that define a type.
    pub fn is_type(self) -> bool {
        matches!(
            self,
            SymbolKind::Class
                | SymbolKind::Struct
                | SymbolKind::Interface
                | SymbolKind::Trait
                | SymbolKind::Enum
                | SymbolKind::Union
                | SymbolKind::Record
                | SymbolKind::Delegate
                | SymbolKind::TypeAlias
        )
    }

    /// True for kinds that are abstract contracts (implemented rather than inherited).
    pub fn is_contract(self) -> bool {
        matches!(self, SymbolKind::Interface | SymbolKind::Trait)
    }
}

/// A declared symbol: namespace, type or member.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct Symbol {
    pub id: SymbolId,
    pub kind: SymbolKind,
    /// Short display name (`OrderService`).
    pub name: String,
    /// Fully qualified name (`Sample.Services.OrderService`).
    pub qualified_name: String,
    pub parent: Option<SymbolId>,
    pub language: Language,
    /// Every place the symbol is declared (partial classes and `impl` blocks have several).
    pub origins: Vec<Span>,
    /// Language-specific traits shown as badges (`static`, `async`, `extension`, `attr:SerializeField`, ...).
    pub flavors: Vec<String>,
    /// Declaration header as written in the source (`public decimal Total(string customer)`).
    pub signature: Option<String>,
    /// Declared type of a field/property/event or return type of a method.
    pub type_text: Option<String>,
    /// True when a [`FlowGraph`] exists for this symbol.
    pub has_flow: bool,
    /// True for symbols that live outside the project (referenced but not declared here).
    pub external: bool,
    /// Documentation written by the author (C# `///` summary, JSDoc, Rust `///`), as plain text.
    #[serde(default)]
    pub doc: Option<String>,
}

/// Relation between two symbols.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[ts(export)]
pub enum StructEdgeKind {
    Contains,
    Inherits,
    Implements,
    Calls,
    Instantiates,
    References,
    Imports,
    Subscribes,
    DependsOn,
    /// A function raises an event (`Handler?.Invoke(...)`).
    Raises,
    /// A type fits an interface without declaring it (TypeScript structural typing).
    Compatible,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct StructEdge {
    pub from: SymbolId,
    pub to: SymbolId,
    pub kind: StructEdgeKind,
}

/// The linked structure of a whole project.
#[derive(Serialize, Deserialize, TS, Clone, Debug, Default, PartialEq)]
#[ts(export)]
pub struct StructureGraph {
    pub symbols: Vec<Symbol>,
    pub edges: Vec<StructEdge>,
}

/// An unresolved reference from a symbol to a type name, resolved by the linker.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct TypeRef {
    pub from: SymbolId,
    /// Name as written in the source, without generic arguments (`IOrderRepository`, `Domain.Order`).
    pub name: String,
    pub relation: TypeRelation,
}

/// How a [`TypeRef`] relates its source symbol to the referenced type.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub enum TypeRelation {
    /// A base-list entry; the linker decides between `Inherits` and `Implements`.
    Base,
    References,
    Instantiates,
}

/// A namespace/module import (`using X;`, `import ... from`, `use ...`).
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct ImportRef {
    /// Namespace or module that performs the import.
    pub from_scope: String,
    /// Imported namespace or module.
    pub target: String,
}

/// External package declared in a project manifest.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct PackageRef {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[ts(export)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
}

/// Everything a language frontend extracts from one source file.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct FileIr {
    pub file: String,
    pub language: Language,
    pub symbols: Vec<Symbol>,
    pub flows: Vec<FlowGraph>,
    pub imports: Vec<ImportRef>,
    pub type_refs: Vec<TypeRef>,
    pub diagnostics: Vec<Diagnostic>,
}

// ---------------------------------------------------------------------------------------------
// Flow graph
// ---------------------------------------------------------------------------------------------

/// The universal vocabulary of flow nodes. A concept decides a node's shape, color and pins;
/// language-specific details are expressed through flavors. Display data lives in `catalog/concepts.toml`.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[ts(export)]
pub enum Concept {
    // Entry and exit
    Entry,
    EventEntry,
    Return,
    Throw,
    PropagateError,
    // Control
    Branch,
    Match,
    #[serde(rename = "Loop.While")]
    LoopWhile,
    #[serde(rename = "Loop.Range")]
    LoopRange,
    #[serde(rename = "Loop.ForEach")]
    LoopForEach,
    #[serde(rename = "Loop.Infinite")]
    LoopInfinite,
    Break,
    Continue,
    Jump,
    /// Conditional compilation (`#if DEBUG`, `#[cfg(...)]`): only one path exists in a build.
    BuildSwitch,
    // Scopes
    Try,
    Scope,
    CriticalSection,
    // Asynchrony
    Await,
    Yield,
    Wait,
    Spawn,
    // Events
    Subscribe,
    Emit,
    // Data
    Literal,
    Var,
    Assign,
    Call,
    Construct,
    GetMember,
    SetMember,
    Index,
    BinaryOp,
    UnaryOp,
    Choose,
    Convert,
    TypeCheck,
    SafeAccess,
    Fallback,
    Compose,
    Destructure,
    Lambda,
    Pipeline,
    /// A shared operation from `catalog/operations.toml` (`+`, `≥`, `and`, `Max`, `To upper case`):
    /// the same node in every language; `Expr::op` names it.
    Operation,
    // Escape hatches
    Opaque,
    Macro,
}

impl Concept {
    /// Every concept, in declaration order. Used to validate the catalog.
    pub const ALL: &'static [Concept] = &[
        Concept::Entry,
        Concept::EventEntry,
        Concept::Return,
        Concept::Throw,
        Concept::PropagateError,
        Concept::Branch,
        Concept::Match,
        Concept::LoopWhile,
        Concept::LoopRange,
        Concept::LoopForEach,
        Concept::LoopInfinite,
        Concept::Break,
        Concept::Continue,
        Concept::Jump,
        Concept::BuildSwitch,
        Concept::Try,
        Concept::Scope,
        Concept::CriticalSection,
        Concept::Await,
        Concept::Yield,
        Concept::Wait,
        Concept::Spawn,
        Concept::Subscribe,
        Concept::Emit,
        Concept::Literal,
        Concept::Var,
        Concept::Assign,
        Concept::Call,
        Concept::Construct,
        Concept::GetMember,
        Concept::SetMember,
        Concept::Index,
        Concept::BinaryOp,
        Concept::UnaryOp,
        Concept::Choose,
        Concept::Convert,
        Concept::TypeCheck,
        Concept::SafeAccess,
        Concept::Fallback,
        Concept::Compose,
        Concept::Destructure,
        Concept::Lambda,
        Concept::Pipeline,
        Concept::Operation,
        Concept::Opaque,
        Concept::Macro,
    ];

    /// The key used in `catalog/concepts.toml` and in JSON (`Loop.ForEach`, `Branch`, ...).
    pub fn key(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    }

    /// True for concepts that end the current execution path (nothing runs after them in the same block).
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Concept::Return | Concept::Throw | Concept::Break | Concept::Continue | Concept::Jump
        )
    }
}

/// Whether a pin carries control flow or a value.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub enum PinKind {
    Exec,
    Data,
}

/// A connection point on a flow node.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq)]
#[ts(export)]
pub struct Pin {
    /// Unique within its node (`in`, `out`, `body`, `true`, `var:total`, ...).
    pub id: String,
    pub kind: PinKind,
    pub label: String,
    /// Type as written in the source, when known.
    pub ty: Option<String>,
}

/// Blueprint purity: impure nodes sit on the execution chain; pure nodes only compute values.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq)]
#[ts(export)]
pub enum Purity {
    Pure,
    Impure,
}

/// An expression tree under a statement node. The frontend shows statements collapsed and
/// expands this tree into pure nodes on demand (statement-level granularity by default).
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct Expr {
    pub concept: Concept,
    /// Short natural-language label (`Where`, `+`, `order.Amount`).
    pub label: String,
    pub span: Span,
    pub flavors: Vec<String>,
    pub children: Vec<Expr>,
    /// Project symbol this expression refers to (a called method, a created type), when a
    /// semantic provider resolved it.
    #[serde(default)]
    pub symbol: Option<SymbolId>,
    /// Display name of the resolved target, including external ones (`System.Console.WriteLine(string)`).
    #[serde(default)]
    pub target: Option<String>,
    /// The shared operation this expression performs (`add`, `greater_or_equal`, `math.max`,
    /// `text.to_upper`): a key of `catalog/operations.toml`. Language-agnostic.
    #[serde(default)]
    pub op: Option<String>,
    /// Type category of the result when known (`bool`, `number`, `text`, `list`): the pin
    /// types of `catalog/operations.toml`.
    #[serde(default)]
    pub ty: Option<String>,
    /// Parameter names of a `Lambda`.
    #[serde(default)]
    pub params: Vec<String>,
}

/// One node of a Blueprint-style flow graph.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct FlowNode {
    pub id: NodeId,
    pub concept: Concept,
    /// Natural-language description (`for each order in _repository.All()`).
    pub label: String,
    /// The original code of the statement header, single line, trimmed.
    pub code: String,
    pub span: Span,
    pub flavors: Vec<String>,
    pub purity: Purity,
    pub inputs: Vec<Pin>,
    pub outputs: Vec<Pin>,
    /// Expression tree of the statement, if any.
    pub expr: Option<Expr>,
    /// Local variables written by this node.
    pub writes: Vec<String>,
    /// Functions this statement calls or types it creates, resolved by a semantic provider.
    #[serde(default)]
    pub calls: Vec<CallTarget>,
}

/// A resolved call or construction target of a flow node.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct CallTarget {
    /// Project symbol, when the target is declared in the project.
    pub symbol: Option<SymbolId>,
    /// Display name (`OrderService.ShipAsync(int)`, `System.Console.WriteLine(string)`).
    pub display: String,
    pub external: bool,
}

/// Whether an edge carries control flow or a value.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub enum FlowEdgeKind {
    Exec,
    Data,
}

#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub struct FlowEdge {
    pub from: NodeId,
    pub from_pin: String,
    pub to: NodeId,
    pub to_pin: String,
    pub kind: FlowEdgeKind,
    /// How a data wire hands its value over, for languages with ownership (Rust). `None` when
    /// the language does not say (C#, TypeScript).
    #[serde(default)]
    pub access: Option<Access>,
}

/// How a value travels along a data wire.
#[derive(Serialize, Deserialize, TS, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[ts(export)]
pub enum Access {
    /// The value moves: the source can no longer use it.
    Move,
    /// A copy is made (numbers, booleans, `Copy` types); the source keeps its value.
    Copy,
    /// Read-only borrow (`&x`): the owner keeps the value.
    Borrow,
    /// Exclusive borrow for changing it (`&mut x`).
    BorrowMut,
}

/// The Blueprint graph of one function, method, constructor or accessor body.
#[derive(Serialize, Deserialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct FlowGraph {
    pub symbol: SymbolId,
    pub name: String,
    pub language: Language,
    pub span: Span,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
}

impl FlowGraph {
    pub fn node(&self, id: &str) -> Option<&FlowNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Exec edges leaving `id`, in insertion order.
    pub fn exec_out<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a FlowEdge> + 'a {
        self.edges
            .iter()
            .filter(move |e| e.kind == FlowEdgeKind::Exec && e.from == id)
    }
}

/// Builds a stable node id from the owning symbol, the concept and a content anchor.
///
/// The anchor is the normalized statement header plus an ordinal among identical headers,
/// so inserting or removing a statement does not change the ids of the other statements.
pub mod semantic;

pub fn stable_id(symbol: &str, concept: Concept, anchor: &str, ordinal: u32) -> NodeId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(symbol.as_bytes());
    hasher.update(&[0]);
    hasher.update(concept.key().as_bytes());
    hasher.update(&[0]);
    hasher.update(anchor.as_bytes());
    hasher.update(&[0]);
    hasher.update(&ordinal.to_le_bytes());
    let hex = hasher.finalize().to_hex();
    format!("n{}", &hex[..12])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concept_keys_use_dotted_loop_names() {
        assert_eq!(Concept::LoopForEach.key(), "Loop.ForEach");
        assert_eq!(Concept::Branch.key(), "Branch");
    }

    #[test]
    fn concept_all_is_complete_and_unique() {
        let mut keys: Vec<String> = Concept::ALL.iter().map(|c| c.key()).collect();
        let len = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), len, "duplicate concept in Concept::ALL");
        assert!(len >= 44);
    }

    #[test]
    fn stable_id_depends_on_anchor_and_ordinal() {
        let a = stable_id("S.M()", Concept::Call, "Foo()", 0);
        assert_eq!(a, stable_id("S.M()", Concept::Call, "Foo()", 0));
        assert_ne!(a, stable_id("S.M()", Concept::Call, "Foo()", 1));
        assert_ne!(a, stable_id("S.M()", Concept::Call, "Bar()", 0));
    }
}
