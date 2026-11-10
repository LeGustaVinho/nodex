//! The types shown in a Blueprint, independent of source syntax and value representation.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Language, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum CollectionKind {
    Array,
    Slice,
    List,
    Dictionary,
    Set,
    Queue,
    Stack,
    Deque,
    PriorityQueue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
#[ts(export)]
pub enum TypeShape {
    Unknown,
    Scalar {
        name: String,
    },
    Collection {
        family: CollectionKind,
        arguments: Vec<TypeShape>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        rank: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        length: Option<String>,
    },
    Named {
        name: String,
        arguments: Vec<TypeShape>,
    },
    Tuple {
        items: Vec<TypeShape>,
    },
    Union {
        items: Vec<TypeShape>,
    },
    Indirect {
        pointer: bool,
        target: Box<TypeShape>,
    },
}

impl TypeShape {
    pub fn scalar(name: &str) -> Self {
        Self::Scalar { name: name.into() }
    }

    pub fn collection(family: CollectionKind, arguments: Vec<Self>) -> Self {
        Self::Collection {
            family,
            arguments,
            rank: None,
            length: None,
        }
    }

    pub fn union(types: impl IntoIterator<Item = Self>) -> Self {
        let mut items = Vec::new();
        for ty in types {
            let alternatives = match ty {
                Self::Union { items } => items,
                other => vec![other],
            };
            for alternative in alternatives {
                if alternative == Self::Unknown {
                    return Self::Unknown;
                }
                if !items.contains(&alternative) {
                    items.push(alternative);
                }
                if items.len() > 8 {
                    return Self::Unknown;
                }
            }
        }
        match items.len() {
            0 => Self::Unknown,
            1 => items.pop().unwrap(),
            _ => Self::Union { items },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TypeOrigin {
    Declared,
    Compiler,
    Syntax,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum CollectionRole {
    Queue,
    Stack,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RoleEvidence {
    pub role: CollectionRole,
    pub insert: Span,
    pub remove: Span,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TypeInfo {
    pub shape: TypeShape,
    pub origin: TypeOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub native: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub role: Option<RoleEvidence>,
}

impl TypeInfo {
    pub fn new(shape: TypeShape, origin: TypeOrigin, native: Option<String>) -> Self {
        Self {
            shape,
            origin,
            native,
            role: None,
        }
    }
}

/// Splits type arguments without splitting nested tuples, arrays or generics.
fn split(text: &str, separator: char) -> Vec<&str> {
    let mut depth = 0usize;
    let mut start = 0;
    let mut parts = Vec::new();
    let mut previous = '\0';
    for (i, ch) in text.char_indices() {
        match ch {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' if previous == '-' => {}
            '>' | ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ch if ch == separator && depth == 0 => {
                parts.push(text[start..i].trim());
                start = i + ch.len_utf8();
            }
            _ => {}
        }
        previous = ch;
    }
    parts.push(text[start..].trim());
    parts
}

fn family(language: Option<Language>, name: &str) -> Option<CollectionKind> {
    use CollectionKind::*;
    let name = name.trim_start_matches("::").trim_start_matches("global::");
    Some(match (language, name) {
        (None, "Array") => Array,
        (None, "Slice") => Slice,
        (None, "List")
        | (Some(Language::Rust), "alloc::vec::Vec" | "std::vec::Vec")
        | (
            Some(Language::Csharp),
            "System.Collections.Generic.List"
            | "System.Collections.Generic.IList"
            | "System.Collections.Generic.IReadOnlyList"
            | "System.Collections.Generic.LinkedList",
        ) => List,
        (None, "Dictionary")
        | (
            Some(Language::Rust),
            "std::collections::HashMap"
            | "std::collections::BTreeMap"
            | "std::collections::hash::map::HashMap"
            | "alloc::collections::btree::map::BTreeMap",
        )
        | (
            Some(Language::Csharp),
            "System.Collections.Generic.Dictionary"
            | "System.Collections.Generic.IDictionary"
            | "System.Collections.Generic.IReadOnlyDictionary",
        ) => Dictionary,
        (None, "Set")
        | (
            Some(Language::Rust),
            "std::collections::HashSet"
            | "std::collections::BTreeSet"
            | "std::collections::hash::set::HashSet"
            | "alloc::collections::btree::set::BTreeSet",
        )
        | (
            Some(Language::Csharp),
            "System.Collections.Generic.HashSet"
            | "System.Collections.Generic.SortedSet"
            | "System.Collections.Generic.ISet",
        ) => Set,
        (None, "Queue") | (Some(Language::Csharp), "System.Collections.Generic.Queue") => Queue,
        (None, "Stack") | (Some(Language::Csharp), "System.Collections.Generic.Stack") => Stack,
        (None, "Deque")
        | (
            Some(Language::Rust),
            "std::collections::VecDeque" | "alloc::collections::vec_deque::VecDeque",
        ) => Deque,
        (None, "PriorityQueue")
        | (
            Some(Language::Rust),
            "std::collections::BinaryHeap" | "alloc::collections::binary_heap::BinaryHeap",
        )
        | (Some(Language::Csharp), "System.Collections.Generic.PriorityQueue") => PriorityQueue,
        (Some(Language::Csharp), "System.Span" | "System.ReadOnlySpan") => Slice,
        _ => return None,
    })
}

fn atom(language: Option<Language>, name: &str) -> Option<&str> {
    let name = name.trim_start_matches("global::");
    Some(match (language, name) {
        (
            None,
            "Bool" | "String" | "Char" | "Number" | "BigInt" | "Decimal" | "Null" | "Undefined"
            | "Object" | "Function" | "Symbol" | "Integer" | "Float" | "Unit" | "Never" | "Int8"
            | "Int16" | "Int32" | "Int64" | "Int128" | "IntSize" | "UInt8" | "UInt16" | "UInt32"
            | "UInt64" | "UInt128" | "UIntSize" | "Float16" | "Float32" | "Float64" | "Float128",
        ) => name,
        (Some(Language::Csharp | Language::Rust), "bool")
        | (Some(Language::Csharp), "System.Boolean")
        | (Some(Language::Typescript), "boolean") => "Bool",
        (Some(Language::Csharp | Language::Typescript), "string")
        | (Some(Language::Csharp), "System.String")
        | (Some(Language::Rust), "str" | "std::string::String" | "alloc::string::String") => {
            "String"
        }
        (Some(Language::Csharp | Language::Rust), "char")
        | (Some(Language::Csharp), "System.Char") => "Char",
        (Some(Language::Typescript), "number") => "Number",
        (Some(Language::Typescript), "bigint") => "BigInt",
        (Some(Language::Typescript), "symbol") => "Symbol",
        (Some(_), "null") => "Null",
        (Some(Language::Typescript), "undefined") => "Undefined",
        (Some(Language::Csharp | Language::Typescript), "object")
        | (Some(Language::Csharp), "System.Object") => "Object",
        (Some(Language::Csharp), "decimal" | "System.Decimal") => "Decimal",
        (Some(Language::Csharp), "sbyte" | "System.SByte") | (Some(Language::Rust), "i8") => "Int8",
        (Some(Language::Csharp), "short" | "System.Int16") | (Some(Language::Rust), "i16") => {
            "Int16"
        }
        (Some(Language::Csharp), "int" | "System.Int32") | (Some(Language::Rust), "i32") => "Int32",
        (Some(Language::Csharp), "long" | "System.Int64") | (Some(Language::Rust), "i64") => {
            "Int64"
        }
        (Some(Language::Csharp), "System.Int128") | (Some(Language::Rust), "i128") => "Int128",
        (Some(Language::Csharp), "nint" | "System.IntPtr") | (Some(Language::Rust), "isize") => {
            "IntSize"
        }
        (Some(Language::Csharp), "byte" | "System.Byte") | (Some(Language::Rust), "u8") => "UInt8",
        (Some(Language::Csharp), "ushort" | "System.UInt16") | (Some(Language::Rust), "u16") => {
            "UInt16"
        }
        (Some(Language::Csharp), "uint" | "System.UInt32") | (Some(Language::Rust), "u32") => {
            "UInt32"
        }
        (Some(Language::Csharp), "ulong" | "System.UInt64") | (Some(Language::Rust), "u64") => {
            "UInt64"
        }
        (Some(Language::Csharp), "System.UInt128") | (Some(Language::Rust), "u128") => "UInt128",
        (Some(Language::Csharp), "nuint" | "System.UIntPtr") | (Some(Language::Rust), "usize") => {
            "UIntSize"
        }
        (Some(Language::Csharp), "System.Half") | (Some(Language::Rust), "f16") => "Float16",
        (Some(Language::Csharp), "float" | "System.Single") | (Some(Language::Rust), "f32") => {
            "Float32"
        }
        (Some(Language::Csharp), "double" | "System.Double") | (Some(Language::Rust), "f64") => {
            "Float64"
        }
        (Some(Language::Rust), "f128") => "Float128",
        (Some(Language::Rust), "{integer}") => "Integer",
        (Some(Language::Rust), "{float}") => "Float",
        (Some(_), "()" | "void" | "System.Void") => "Unit",
        (Some(_), "!" | "never") => "Never",
        _ => return None,
    })
}

/// Parses native type text without assuming a bare collection name belongs to the standard library.
pub fn native_shape(language: Language, text: &str) -> TypeShape {
    parse(Some(language), text, 0, true)
}

/// A written name alone does not establish the identity of a standard collection.
pub fn syntax_shape(language: Language, text: &str) -> TypeShape {
    parse(Some(language), text, 0, false)
}

/// Parses the portable type descriptor emitted by a trace runtime.
pub fn portable_shape(text: &str) -> TypeShape {
    parse(None, text, 0, true)
}

fn parse(
    language: Option<Language>,
    text: &str,
    depth: usize,
    recognize_collections: bool,
) -> TypeShape {
    use CollectionKind::{Array, Slice};
    use TypeShape::*;
    let text = text.trim().trim_start_matches(':').trim();
    if depth >= 8
        || text.len() > 2048
        || matches!(
            text,
            "" | "?" | "any" | "unknown" | "_" | "var" | "dynamic" | "{unknown}"
        )
    {
        return Unknown;
    }
    let inner = |ty| parse(language, ty, depth + 1, recognize_collections);
    if let Some(name) = atom(language, text) {
        return TypeShape::scalar(name);
    }
    let alternatives = split(text, '|');
    if alternatives.len() > 1 {
        return TypeShape::union(alternatives.into_iter().map(inner));
    }
    if let Some(ty) = text
        .strip_prefix("ref readonly ")
        .or_else(|| text.strip_prefix("ref "))
    {
        return Indirect {
            pointer: false,
            target: Box::new(inner(ty)),
        };
    }
    if let Some(ty) = text.strip_prefix('&') {
        let mut ty = ty.trim_start();
        if ty.starts_with('\'') {
            ty = ty
                .split_once(char::is_whitespace)
                .map_or("", |(_, rest)| rest.trim_start());
        }
        return Indirect {
            pointer: false,
            target: Box::new(inner(ty.strip_prefix("mut ").unwrap_or(ty))),
        };
    }
    if let Some(ty) = text
        .strip_prefix("*const ")
        .or_else(|| text.strip_prefix("*mut "))
    {
        return Indirect {
            pointer: true,
            target: Box::new(inner(ty)),
        };
    }
    if let Some(ty) = text.strip_suffix('?') {
        return TypeShape::union([inner(ty), TypeShape::scalar("Null")]);
    }
    if let Some(ty) = text.strip_prefix('[').and_then(|ty| ty.strip_suffix(']')) {
        if language == Some(Language::Rust) {
            let parts = split(ty, ';');
            let length = parts.get(1).map(|value| (*value).to_owned());
            return Collection {
                family: if length.is_some() { Array } else { Slice },
                arguments: vec![inner(parts[0])],
                rank: None,
                length,
            };
        }
        return Tuple {
            items: split(ty, ',')
                .into_iter()
                .filter(|part| !part.is_empty())
                .map(inner)
                .collect(),
        };
    }
    if let Some((ty, rank)) = text.strip_suffix(']').and_then(|ty| ty.rsplit_once('['))
        && rank.bytes().all(|byte| byte == b',')
    {
        return Collection {
            family: Array,
            arguments: vec![inner(ty)],
            rank: (rank.len() > 0).then_some(rank.len() as u32 + 1),
            length: None,
        };
    }
    if let Some(ty) = text
        .strip_suffix('*')
        .filter(|_| !text.starts_with("delegate*"))
    {
        return Indirect {
            pointer: true,
            target: Box::new(inner(ty)),
        };
    }
    if let Some(ty) = text.strip_prefix('(').and_then(|ty| ty.strip_suffix(')')) {
        let parts = split(ty, ',');
        if parts.len() == 1 {
            return inner(parts[0]);
        }
        return Tuple {
            items: parts
                .into_iter()
                .filter(|part| !part.is_empty())
                .map(inner)
                .collect(),
        };
    }
    let named = language.is_none() && text.starts_with("type:");
    let text = if named {
        text.strip_prefix("type:").unwrap_or(text)
    } else {
        text
    };
    let (name, mut arguments) = text
        .strip_suffix('>')
        .and_then(|ty| ty.split_once('<'))
        .map_or((text, Vec::new()), |(name, args)| {
            (
                name.trim(),
                split(args, ',').into_iter().take(16).map(inner).collect(),
            )
        });
    let name = if language == Some(Language::Rust) {
        name.trim_end_matches("::")
    } else {
        name
    };
    if !named && language.is_none() && matches!(name, "Ref" | "Pointer") {
        return Indirect {
            pointer: name == "Pointer",
            target: Box::new(arguments.into_iter().next().unwrap_or(Unknown)),
        };
    }
    if recognize_collections
        && !named
        && let Some(family) = family(language, name)
    {
        if language == Some(Language::Rust) {
            let arity = if family == CollectionKind::Dictionary {
                2
            } else {
                1
            };
            arguments.resize(arity, Unknown);
        }
        return TypeShape::collection(family, arguments);
    }
    let name = name.trim_start_matches("global::");
    if !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | ':' | '.' | '$' | '#'))
    {
        Named {
            name: name
                .rsplit("::")
                .next()
                .unwrap_or(name)
                .rsplit('.')
                .next()
                .unwrap_or(name)
                .into(),
            arguments,
        }
    } else {
        Unknown
    }
}
