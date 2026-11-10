//! Syntax evidence and point-sensitive fallback types for Blueprint data pins.

use std::collections::{HashMap, HashSet, VecDeque};

use nodex_ir::types::{
    CollectionKind, TypeInfo, TypeOrigin, TypeShape, native_shape, syntax_shape,
};
use nodex_ir::{Concept, Expr, FileIr, FlowEdgeKind, FlowGraph, Language, Pin, PinKind, SymbolId};
use tree_sitter::Node;

use crate::kit::ExecCursor;
use crate::syntax::{Src, named_children};

type Range = (u32, u32);

/// The real exits of compound statements with a visual continuation port.
pub type Continuations = HashMap<ExecCursor, Vec<ExecCursor>>;

fn range(node: Node<'_>) -> Range {
    (node.start_byte() as u32, node.end_byte() as u32)
}

fn syntax_info(shape: TypeShape) -> TypeInfo {
    TypeInfo::new(shape, TypeOrigin::Syntax, None)
}

fn declared(language: Language, text: &str) -> TypeInfo {
    TypeInfo::new(
        syntax_shape(language, text),
        TypeOrigin::Declared,
        Some(text.into()),
    )
}

/// Types the written expression establishes without a compiler.
pub fn expression_type(language: Language, src: Src<'_>, node: Node<'_>) -> Option<TypeShape> {
    expression(language, src, node, 0)
}

fn csharp_common_type(types: Vec<TypeShape>) -> TypeShape {
    fn converts(from: &TypeShape, to: &TypeShape) -> bool {
        if from == to {
            return true;
        }
        if from == &TypeShape::scalar("Null") {
            return matches!(to, TypeShape::Scalar { name } if matches!(name.as_str(), "String" | "Object"))
                || matches!(
                    to,
                    TypeShape::Collection {
                        family: CollectionKind::Array,
                        ..
                    }
                );
        }
        let (TypeShape::Scalar { name: from }, TypeShape::Scalar { name: to }) = (from, to) else {
            return false;
        };
        let wider: &[&str] = match from.as_str() {
            "Int8" => &["Int16", "Int32", "Int64"],
            "UInt8" => &["Int16", "UInt16", "Int32", "UInt32", "Int64", "UInt64"],
            "Int16" => &["Int32", "Int64"],
            "UInt16" => &["Int32", "UInt32", "Int64", "UInt64"],
            "Char" => &["UInt16", "Int32", "UInt32", "Int64", "UInt64"],
            "Int32" => &["Int64"],
            "UInt32" => &["Int64", "UInt64"],
            "Int64" | "UInt64" => &[],
            "Float32" => return to == "Float64",
            _ => return false,
        };
        wider.contains(&to.as_str()) || matches!(to.as_str(), "Float32" | "Float64" | "Decimal")
    }

    let Some(common) = types
        .iter()
        .find(|to| types.iter().all(|from| converts(from, to)))
    else {
        return TypeShape::Unknown;
    };
    if common != &TypeShape::scalar("Null") && types.contains(&TypeShape::scalar("Null")) {
        TypeShape::union([common.clone(), TypeShape::scalar("Null")])
    } else {
        common.clone()
    }
}

fn expression(language: Language, src: Src<'_>, node: Node<'_>, depth: u8) -> Option<TypeShape> {
    if depth >= 8 {
        return None;
    }
    let child_type =
        |node| expression(language, src, node, depth + 1).unwrap_or(TypeShape::Unknown);
    let scalar = |name| Some(TypeShape::scalar(name));
    let text = src.text(node);
    match node.kind() {
        "parenthesized_expression" | "satisfies_expression" => named_children(node)
            .into_iter()
            .find(|child| !child.is_extra())
            .and_then(|child| expression(language, src, child, depth + 1)),
        "argument" => named_children(node)
            .into_iter()
            .rfind(|child| !child.is_extra())
            .and_then(|child| expression(language, src, child, depth + 1)),
        "true" | "false" | "boolean_literal" => scalar("Bool"),
        "string"
        | "template_string"
        | "string_literal"
        | "raw_string_literal"
        | "verbatim_string_literal"
        | "interpolated_string_expression" => {
            let shape = if language == Language::Rust
                && (text.starts_with("b\"") || text.starts_with("br"))
            {
                TypeShape::collection(CollectionKind::Array, vec![TypeShape::scalar("UInt8")])
            } else if language == Language::Rust
                && (text.starts_with("c\"") || text.starts_with("cr\"") || text.starts_with("cr#"))
            {
                TypeShape::Named {
                    name: "CStr".into(),
                    arguments: Vec::new(),
                }
            } else {
                TypeShape::scalar("String")
            };
            Some(if language == Language::Rust {
                TypeShape::Indirect {
                    pointer: false,
                    target: Box::new(shape),
                }
            } else {
                shape
            })
        }
        "null" | "null_literal" => scalar("Null"),
        "char_literal" | "character_literal" => {
            scalar(if language == Language::Rust && text.starts_with("b'") {
                "UInt8"
            } else {
                "Char"
            })
        }
        "ternary_expression" | "conditional_expression" => {
            let alternatives = ["consequence", "alternative"].map(|field| {
                node.child_by_field_name(field)
                    .map(child_type)
                    .unwrap_or(TypeShape::Unknown)
            });
            Some(if language == Language::Csharp {
                csharp_common_type(alternatives.into())
            } else {
                TypeShape::union(alternatives)
            })
        }
        "number" if language == Language::Typescript => scalar(if text.ends_with('n') {
            "BigInt"
        } else {
            "Number"
        }),
        "integer_literal" | "float_literal" if language == Language::Rust => {
            for suffix in [
                "isize", "usize", "i128", "u128", "i64", "u64", "i32", "u32", "i16", "u16", "i8",
                "u8", "f128", "f64", "f32", "f16",
            ] {
                if text.ends_with(suffix) && (!text.starts_with("0x") || !suffix.starts_with('f')) {
                    return Some(native_shape(language, suffix));
                }
            }
            scalar(if node.kind() == "integer_literal" {
                "Integer"
            } else {
                "Float"
            })
        }
        "integer_literal" => {
            let text = text.to_ascii_lowercase().replace('_', "");
            let digits = text.trim_end_matches(['u', 'l']);
            let (digits, radix) = digits
                .strip_prefix("0x")
                .map(|s| (s, 16))
                .or_else(|| digits.strip_prefix("0b").map(|s| (s, 2)))
                .unwrap_or((digits, 10));
            let number = u64::from_str_radix(digits, radix).ok()?;
            let unsigned = text.ends_with('u') || text.ends_with("ul");
            let long = text.ends_with('l') || text.ends_with("lu");
            scalar(match (unsigned, long, number) {
                (false, false, n) if n <= i32::MAX as u64 => "Int32",
                (_, false, n) if n <= u32::MAX as u64 => "UInt32",
                (false, _, n) if n <= i64::MAX as u64 => "Int64",
                _ => "UInt64",
            })
        }
        "real_literal" => scalar(match text.as_bytes().last() {
            Some(b'f' | b'F') => "Float32",
            Some(b'm' | b'M') => "Decimal",
            _ => "Float64",
        }),
        "array" | "array_expression" | "implicit_array_creation_expression" => {
            let elements = if node.kind() == "implicit_array_creation_expression" {
                named_children(node)
                    .into_iter()
                    .find(|child| child.kind() == "initializer_expression")
                    .map(named_children)
                    .unwrap_or_default()
            } else {
                named_children(node)
            };
            let repeated = node.child_by_field_name("length");
            let elements: Vec<_> = elements
                .into_iter()
                .filter(|child| !child.is_extra() && repeated != Some(*child))
                .collect();
            let count = elements.len();
            let types = elements.into_iter().map(child_type);
            let element = if language == Language::Csharp {
                csharp_common_type(types.collect())
            } else {
                TypeShape::union(types)
            };
            Some(TypeShape::Collection {
                family: CollectionKind::Array,
                arguments: vec![element],
                rank: None,
                length: (language == Language::Rust).then(|| {
                    repeated
                        .map(|n| src.text(n).into())
                        .unwrap_or_else(|| count.to_string())
                }),
            })
        }
        "tuple_expression" => Some(TypeShape::Tuple {
            items: named_children(node)
                .into_iter()
                .filter(|child| !child.is_extra())
                .map(child_type)
                .collect(),
        }),
        "object" | "anonymous_object_creation_expression" => scalar("Object"),
        "arrow_function" | "function_expression" | "generator_function" | "closure_expression" => {
            scalar("Function")
        }
        "reference_expression" if language == Language::Rust => {
            let target = node
                .child_by_field_name("value")
                .map(child_type)
                .unwrap_or(TypeShape::Unknown);
            let pointer = crate::syntax::children(node)
                .iter()
                .any(|child| child.kind() == "raw");
            Some(TypeShape::Indirect {
                pointer,
                target: Box::new(target),
            })
        }
        "object_creation_expression" | "struct_expression" | "new_expression" => {
            let ty = node
                .child_by_field_name("type")
                .or_else(|| node.child_by_field_name("name"))
                .or_else(|| {
                    node.child_by_field_name("constructor")
                        .filter(|name| matches!(name.kind(), "identifier" | "member_expression"))
                })?;
            // Written constructor names do not establish standard-library identity.
            Some(syntax_shape(language, src.text(ty)))
        }
        "array_creation_expression" => {
            let mut element = node.child_by_field_name("type")?;
            let mut ranks = Vec::new();
            while element.kind() == "array_type" {
                let rank = element.child_by_field_name("rank")?;
                ranks.push(
                    1 + crate::syntax::children(rank)
                        .iter()
                        .filter(|child| child.kind() == ",")
                        .count() as u32,
                );
                element = element.child_by_field_name("type")?;
            }
            // The leftmost rank in a C# array creation describes the outer array.
            let shape =
                ranks
                    .into_iter()
                    .fold(syntax_shape(language, src.text(element)), |item, rank| {
                        TypeShape::Collection {
                            family: CollectionKind::Array,
                            arguments: vec![item],
                            rank: (rank > 1).then_some(rank),
                            length: None,
                        }
                    });
            Some(shape)
        }
        _ => None,
    }
}

fn pattern_leaves<'tree>(pattern: Node<'tree>, out: &mut Vec<Node<'tree>>) {
    match pattern.kind() {
        "identifier"
        | "implicit_parameter"
        | "shorthand_property_identifier_pattern"
        | "private_property_identifier" => out.push(pattern),
        "pair_pattern" | "assignment_pattern" | "object_assignment_pattern" => {
            if let Some(value) = pattern
                .child_by_field_name("value")
                .or_else(|| pattern.child_by_field_name("left"))
            {
                pattern_leaves(value, out);
            }
        }
        "tuple_pattern"
        | "array_pattern"
        | "object_pattern"
        | "rest_pattern"
        | "mut_pattern"
        | "reference_pattern"
        | "ref_pattern"
        | "tuple_struct_pattern"
        | "tuple_designation"
        | "parenthesized_variable_designation"
        | "single_variable_designation"
        | "declaration_expression" => {
            let constructor = pattern.child_by_field_name("type");
            for child in named_children(pattern)
                .into_iter()
                .filter(|child| Some(*child) != constructor)
            {
                pattern_leaves(child, out);
            }
        }
        _ => {}
    }
}

/// Positional children, including elisions in JavaScript arrays.
fn sequence_items(node: Node<'_>) -> Vec<Option<Node<'_>>> {
    let mut items = Vec::new();
    let mut item = None;
    for child in crate::syntax::children(node) {
        if child.is_extra() {
            continue;
        }
        if child.kind() == "," {
            items.push(item.take());
        } else if child.is_named() {
            item = Some(child);
        }
    }
    if item.is_some() {
        items.push(item);
    }
    items
}

fn value_operand(mut node: Node<'_>) -> Node<'_> {
    while matches!(
        node.kind(),
        "parenthesized_expression" | "argument" | "type_annotation"
    ) {
        let Some(child) = named_children(node)
            .into_iter()
            .rfind(|child| !child.is_extra())
        else {
            break;
        };
        node = child;
    }
    node
}

/// A function value invoked directly at this site. Generator bodies run on iteration.
pub fn immediately_called_script_function(call: Node<'_>) -> Option<Node<'_>> {
    if call.kind() != "call_expression" {
        return None;
    }
    let callee = value_operand(call.child_by_field_name("function")?);
    matches!(
        callee.kind(),
        "arrow_function" | "function_expression" | "function"
    )
    .then_some(callee)
}

fn script_function_boundary(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "arrow_function"
            | "function_expression"
            | "function"
            | "function_declaration"
            | "generator_function"
            | "generator_function_declaration"
            | "method_definition"
            | "class"
            | "class_declaration"
            | "abstract_class_declaration"
    )
}

fn script_pattern_names<'a>(src: Src<'a>, pattern: Node<'_>, names: &mut Vec<&'a str>) {
    let mut leaves = Vec::new();
    pattern_leaves(pattern, &mut leaves);
    names.extend(leaves.into_iter().map(|leaf| src.text(leaf)));
}

fn script_parameters(function: Node<'_>) -> Vec<Node<'_>> {
    if let Some(param) = function.child_by_field_name("parameter") {
        return vec![param];
    }
    function
        .child_by_field_name("parameters")
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|param| !param.is_extra())
        .collect()
}

fn script_function_locals<'a>(src: Src<'a>, function: Node<'_>, names: &mut Vec<&'a str>) {
    if let Some(name) = function.child_by_field_name("name") {
        script_pattern_names(src, name, names);
    }
    for param in script_parameters(function) {
        script_pattern_names(
            src,
            param.child_by_field_name("pattern").unwrap_or(param),
            names,
        );
    }
    let mut nodes: Vec<_> = function.child_by_field_name("body").into_iter().collect();
    while let Some(node) = nodes.pop() {
        if script_function_boundary(node) {
            continue;
        }
        if node.kind() == "variable_declaration" {
            for name in named_children(node)
                .into_iter()
                .filter_map(|child| child.child_by_field_name("name"))
            {
                script_pattern_names(src, name, names);
            }
        }
        if node.kind() == "for_in_statement"
            && node
                .child_by_field_name("kind")
                .is_some_and(|kind| kind.kind() == "var")
            && let Some(pattern) = node.child_by_field_name("left")
        {
            script_pattern_names(src, pattern, names);
        }
        nodes.extend(named_children(node));
    }
}

/// Captured bindings a directly invoked function may change. Local declarations and parameters
/// shadow the caller even when the body is too complex to infer its effects precisely.
pub fn immediate_script_writes<'a>(src: Src<'a>, function: Node<'_>) -> Vec<&'a str> {
    fn visit<'a>(
        src: Src<'a>,
        node: Node<'_>,
        shadowed: &mut Vec<&'a str>,
        writes: &mut Vec<&'a str>,
    ) {
        if script_function_boundary(node) {
            return;
        }
        let outer = shadowed.len();
        let statements = match node.kind() {
            "statement_block" => named_children(node),
            "switch_body" => named_children(node)
                .into_iter()
                .flat_map(named_children)
                .collect(),
            "for_statement" => node
                .child_by_field_name("initializer")
                .into_iter()
                .collect(),
            _ => Vec::new(),
        };
        for declaration in statements {
            if declaration.kind() == "lexical_declaration" {
                for name in named_children(declaration)
                    .into_iter()
                    .filter_map(|child| child.child_by_field_name("name"))
                {
                    script_pattern_names(src, name, shadowed);
                }
            } else if matches!(
                declaration.kind(),
                "class_declaration" | "function_declaration" | "generator_function_declaration"
            ) && let Some(name) = declaration.child_by_field_name("name")
            {
                script_pattern_names(src, name, shadowed);
            }
        }
        if node.kind() == "catch_clause"
            && let Some(pattern) = node.child_by_field_name("parameter")
        {
            script_pattern_names(src, pattern, shadowed);
        }
        if node.kind() == "for_in_statement"
            && node
                .child_by_field_name("kind")
                .is_some_and(|kind| matches!(kind.kind(), "let" | "const"))
            && let Some(pattern) = node.child_by_field_name("left")
        {
            script_pattern_names(src, pattern, shadowed);
        }
        let target = match node.kind() {
            "assignment_expression" | "augmented_assignment_expression" => {
                node.child_by_field_name("left")
            }
            "update_expression" => node.child_by_field_name("argument"),
            "for_in_statement" if node.child_by_field_name("kind").is_none() => {
                node.child_by_field_name("left")
            }
            _ => None,
        };
        if let Some(target) = target {
            let mut names = Vec::new();
            script_pattern_names(src, value_operand(target), &mut names);
            writes.extend(names.into_iter().filter(|name| !shadowed.contains(name)));
        }
        for child in named_children(node) {
            visit(src, child, shadowed, writes);
        }
        if let Some(nested) = immediately_called_script_function(node) {
            visit_function(src, nested, shadowed, writes);
        }
        shadowed.truncate(outer);
    }
    fn visit_function<'a>(
        src: Src<'a>,
        function: Node<'_>,
        shadowed: &mut Vec<&'a str>,
        writes: &mut Vec<&'a str>,
    ) {
        let outer = shadowed.len();
        script_function_locals(src, function, shadowed);
        for param in script_parameters(function) {
            visit(src, param, shadowed, writes);
        }
        if let Some(body) = function.child_by_field_name("body") {
            visit(src, body, shadowed, writes);
        }
        shadowed.truncate(outer);
    }
    let mut writes = Vec::new();
    visit_function(src, function, &mut Vec::new(), &mut writes);
    writes
}

/// Find the written element or property received by one binding in a pattern.
pub fn project_pattern<'t>(
    src: Src<'_>,
    pattern: Node<'t>,
    value: Node<'t>,
    leaf: Node<'t>,
) -> Option<Node<'t>> {
    let value = value_operand(value);
    if pattern.id() == leaf.id() {
        return Some(value);
    }
    let contains_leaf =
        |node: Node| node.start_byte() <= leaf.start_byte() && leaf.end_byte() <= node.end_byte();
    match pattern.kind() {
        "mut_pattern" => {
            let inner = named_children(pattern)
                .into_iter()
                .find(|node| contains_leaf(*node))?;
            project_pattern(src, inner, value, leaf)
        }
        "tuple_pattern" | "array_pattern"
            if matches!(
                value.kind(),
                "tuple_expression" | "tuple_type" | "array" | "array_expression"
            ) =>
        {
            if value.child_by_field_name("length").is_some() {
                return None;
            }
            let children = sequence_items(pattern);
            let i = children
                .iter()
                .position(|item| item.is_some_and(contains_leaf))?;
            let inner = children[i]?;
            let values = sequence_items(value);
            let item = values.get(i).copied().flatten()?;
            if children[..=i]
                .iter()
                .flatten()
                .any(|child| matches!(child.kind(), "rest_pattern" | "remaining_field_pattern"))
                || values[..=i]
                    .iter()
                    .flatten()
                    .any(|child| child.kind() == "spread_element")
            {
                return None;
            }
            project_pattern(src, inner, item, leaf)
        }
        "object_pattern" if value.kind() == "object" => {
            let entry = named_children(pattern)
                .into_iter()
                .find(|node| contains_leaf(*node))?;
            let (key, inner) = match entry.kind() {
                "pair_pattern" => (
                    entry.child_by_field_name("key")?,
                    entry.child_by_field_name("value")?,
                ),
                "shorthand_property_identifier_pattern" => (entry, entry),
                _ => return None,
            };
            let key_name = |node: Node| {
                matches!(
                    node.kind(),
                    "identifier"
                        | "property_identifier"
                        | "shorthand_property_identifier_pattern"
                        | "string"
                        | "number"
                )
                .then(|| src.text(node))
                .filter(|name| !name.contains('\\'))
                .map(|name| name.trim_matches(['\'', '"']))
            };
            let key = key_name(key)?;
            for property in named_children(value)
                .into_iter()
                .rev()
                .filter(|child| !child.is_extra())
            {
                if property.kind() != "pair" {
                    return None;
                }
                if key_name(property.child_by_field_name("key")?)? == key {
                    return project_pattern(
                        src,
                        inner,
                        property.child_by_field_name("value")?,
                        leaf,
                    );
                }
            }
            None
        }
        _ => None,
    }
}

fn parameter_pins(node: Node<'_>, language: Language, src: Src<'_>) -> Vec<Pin> {
    let Some(parameters) = node
        .child_by_field_name("parameters")
        .or_else(|| node.child_by_field_name("parameter"))
    else {
        return Vec::new();
    };
    let parameters = match parameters.kind() {
        "formal_parameters" | "parameter_list" | "closure_parameters" => named_children(parameters),
        _ => vec![parameters],
    };
    let mut pins = Vec::new();
    for parameter in parameters {
        let pattern = parameter
            .child_by_field_name("pattern")
            .or_else(|| parameter.child_by_field_name("name"))
            .unwrap_or(parameter);
        let mut names = Vec::new();
        pattern_leaves(pattern, &mut names);
        let native = parameter.child_by_field_name("type").map(|ty| src.norm(ty));
        for name in names {
            let label = src.text(name).to_owned();
            let ty = (name == pattern).then(|| native.clone()).flatten();
            pins.push(Pin {
                id: format!("p:{label}"),
                kind: PinKind::Data,
                label,
                declaration: Some(src.span(name)),
                type_info: ty.as_deref().map(|text| declared(language, text)),
                ty,
                value_info: None,
                binding: None,
            });
        }
    }
    pins
}

pub fn annotate(
    ir: &mut FileIr,
    root: Node<'_>,
    src: Src<'_>,
    continuations: &HashMap<SymbolId, Continuations>,
) {
    let language = ir.language;
    let mut expressions = HashMap::new();
    let mut parameters = HashMap::new();
    let mut bindings = Vec::new();
    let mut compound = HashMap::new();
    let mut nodes = vec![root];
    while let Some(node) = nodes.pop() {
        if node.is_error() || node.is_extra() || node.is_missing() || node.kind() == "token_tree" {
            continue;
        }
        nodes.extend(named_children(node));
        if let Some(shape) = expression(language, src, node, 0) {
            expressions.insert(range(node), syntax_info(shape));
        }
        if matches!(
            node.kind(),
            "arrow_function"
                | "function_expression"
                | "generator_function"
                | "closure_expression"
                | "lambda_expression"
                | "anonymous_method_expression"
        ) {
            let pins = parameter_pins(node, language, src);
            if node.kind() == "arrow_function"
                && let Some(param) = node
                    .child_by_field_name("parameter")
                    .filter(|param| param.kind() == "identifier")
            {
                bindings.push((src.span(param), src.text(param), None));
            }
            if !pins.is_empty() {
                parameters.insert(range(node), pins);
            }
        }
        if let Some((name, annotation, initializer, _)) = super::value_shapes::declaration(node) {
            let info = annotation
                .filter(|ty| {
                    !matches!(src.text(*ty).trim(), "var" | "self" | "&self" | "&mut self")
                })
                .map(|ty| declared(language, &src.norm(ty)))
                .or_else(|| {
                    initializer
                        .and_then(|value| expression(language, src, value, 0))
                        .map(syntax_info)
                });
            bindings.push((src.span(name), src.text(name), info));
        } else if matches!(
            node.kind(),
            "variable_declarator"
                | "let_declaration"
                | "parameter"
                | "required_parameter"
                | "optional_parameter"
                | "catch_clause"
                | "for_in_statement"
                | "single_variable_designation"
                | "declaration_expression"
        ) {
            let pattern = node
                .child_by_field_name("pattern")
                .or_else(|| node.child_by_field_name("name"))
                .or_else(|| node.child_by_field_name("parameter"))
                .or_else(|| node.child_by_field_name("left"))
                .or_else(|| {
                    named_children(node)
                        .into_iter()
                        .find(|child| child.kind() == "tuple_pattern")
                });
            let mut leaves = Vec::new();
            let pattern = pattern.unwrap_or(node);
            pattern_leaves(pattern, &mut leaves);
            let initializer = node.child_by_field_name("value").or_else(|| {
                (language == Language::Csharp && node.kind() == "variable_declarator")
                    .then(|| {
                        named_children(node).into_iter().find(|child| {
                            child.start_byte() >= pattern.end_byte() && !child.is_extra()
                        })
                    })
                    .flatten()
            });
            for name in leaves {
                let info = node
                    .child_by_field_name("type")
                    .and_then(|ty| project_pattern(src, pattern, ty, name))
                    .map(|ty| declared(language, &src.norm(ty)))
                    .or_else(|| {
                        initializer
                            .and_then(|value| project_pattern(src, pattern, value, name))
                            .and_then(|value| expression_type(language, src, value))
                            .map(syntax_info)
                    });
                bindings.push((src.span(name), src.text(name), info));
            }
        }
        if language == Language::Typescript
            && matches!(
                node.kind(),
                "switch_statement"
                    | "try_statement"
                    | "while_statement"
                    | "do_statement"
                    | "for_statement"
                    | "for_in_statement"
            )
        {
            compound.insert(node.start_byte() as u32, node);
        }
    }
    bindings.sort_by_key(|(span, _, _)| (span.start_byte, span.end_byte));
    bindings.dedup_by(|left, right| left.0 == right.0);
    for symbol in &mut ir.symbols {
        if let Some(ty) = &symbol.type_text {
            symbol.type_info = Some(declared(language, ty));
        } else if matches!(
            symbol.kind,
            nodex_ir::SymbolKind::Field | nodex_ir::SymbolKind::Property
        ) {
            symbol.type_info = symbol.origins.iter().find_map(|origin| {
                bindings
                    .iter()
                    .find(|(span, name, _)| {
                        *name == symbol.name
                            && origin.start_byte <= span.start_byte
                            && span.end_byte <= origin.end_byte
                    })
                    .and_then(|(_, _, info)| info.clone())
            });
        }
    }
    for flow in &mut ir.flows {
        for node in &mut flow.nodes {
            let first =
                bindings.partition_point(|(span, _, _)| span.start_byte < node.span.start_byte);
            for pin in &mut node.outputs {
                let Some(name) = pin.id.strip_prefix("var:") else {
                    continue;
                };
                let mut matches = bindings[first..]
                    .iter()
                    .take_while(|(span, _, _)| span.start_byte < node.span.end_byte)
                    .filter(|(span, label, _)| {
                        *label == name
                            && span.end_byte <= node.span.end_byte
                            && pin
                                .declaration
                                .as_ref()
                                .is_none_or(|declaration| declaration == span)
                    });
                if let Some((span, _, info)) = matches.next().filter(|_| matches.next().is_none()) {
                    pin.declaration.get_or_insert_with(|| span.clone());
                    pin.type_info = info.clone();
                }
                if pin.type_info.is_none() && language != Language::Typescript {
                    pin.type_info = pin
                        .ty
                        .as_deref()
                        .map(|ty| syntax_info(syntax_shape(language, ty)));
                }
            }
            if let Some(expr) = &mut node.expr {
                annotate_expression(expr, &expressions, &parameters);
            }
        }
        if language == Language::Typescript {
            track_assignments(
                flow,
                root,
                src,
                &expressions,
                continuations.get(&flow.symbol),
                &compound,
            );
        } else {
            carry_reads(flow);
        }
    }
}

fn annotate_expression(
    expr: &mut Expr,
    types: &HashMap<Range, TypeInfo>,
    params: &HashMap<Range, Vec<Pin>>,
) {
    let range = (expr.span.start_byte, expr.span.end_byte);
    expr.type_info = types.get(&range).cloned();
    if expr.concept == Concept::Lambda {
        expr.param_pins = params.get(&range).cloned();
    }
    for child in &mut expr.children {
        annotate_expression(child, types, params);
    }
}

fn binding_read(expr: &Expr) -> bool {
    expr.concept == Concept::Var
        && !expr.syntax_sites.iter().flatten().any(|site| {
            matches!(
                site.key.as_str(),
                "borrow" | "borrow-mut" | "raw-borrow" | "deref" | "try"
            )
        })
}

fn update_reads(expr: &mut Expr, reads: &HashMap<&str, Option<TypeInfo>>) {
    if expr.concept == Concept::Lambda {
        return;
    }
    if binding_read(expr)
        && expr.type_info.as_ref().is_none_or(|info| {
            info.origin == TypeOrigin::Syntax && info.shape == TypeShape::Unknown
        })
        && let Some(info) = reads.get(expr.label.as_str())
    {
        expr.type_info = info.clone();
    }
    for child in &mut expr.children {
        update_reads(child, reads);
    }
}

/// Retain exact expression facts at reads; use the defining pin for other languages.
pub fn carry_reads(flow: &mut FlowGraph) {
    let definitions: HashMap<_, _> = flow
        .nodes
        .iter()
        .flat_map(|node| {
            node.outputs.iter().filter_map(|pin| {
                pin.type_info
                    .as_ref()
                    .map(|ty| ((node.id.as_str(), pin.id.as_str()), ty))
            })
        })
        .collect();
    let mut reads: HashMap<String, HashMap<String, Option<TypeInfo>>> = HashMap::new();
    for edge in flow
        .edges
        .iter()
        .filter(|edge| edge.kind == FlowEdgeKind::Data)
    {
        if let Some(name) = edge.to_pin.strip_prefix("use:")
            && let Some(info) = definitions.get(&(edge.from.as_str(), edge.from_pin.as_str()))
        {
            reads
                .entry(edge.to.clone())
                .or_default()
                .insert(name.into(), Some((*info).clone()));
        }
    }
    let mut write_types: HashMap<(String, String), TypeInfo> = HashMap::new();
    if flow.language != Language::Typescript {
        for node in &flow.nodes {
            for pin in &node.outputs {
                let Some(name) = pin.id.strip_prefix("write:") else {
                    continue;
                };
                let ty = node
                    .binding_writes
                    .iter()
                    .find(|binding| binding.pin.strip_prefix("var:") == Some(name))
                    .and_then(|binding| {
                        definitions
                            .get(&(binding.node.as_str(), binding.pin.as_str()))
                            .copied()
                    })
                    .cloned()
                    .or_else(|| {
                        pin.ty.as_deref().map(|ty| {
                            TypeInfo::new(
                                syntax_shape(flow.language, ty),
                                TypeOrigin::Syntax,
                                Some(ty.into()),
                            )
                        })
                    });
                if let Some(info) = ty {
                    write_types.insert((node.id.clone(), pin.id.clone()), info);
                }
            }
        }
    }
    fn exact_reads<'a>(expr: &'a Expr, types: &mut HashMap<&'a str, Vec<&'a TypeInfo>>) {
        if expr.concept == Concept::Lambda {
            return;
        }
        if binding_read(expr)
            && let Some(info) = &expr.type_info
        {
            types.entry(&expr.label).or_default().push(info);
        }
        for child in &expr.children {
            exact_reads(child, types);
        }
    }
    for node in &mut flow.nodes {
        for pin in node
            .outputs
            .iter_mut()
            .filter(|pin| pin.id.starts_with("write:"))
        {
            if let Some(info) = write_types.remove(&(node.id.clone(), pin.id.clone())) {
                pin.type_info = Some(info);
            }
        }
        let defaults: HashMap<_, _> = reads
            .get(&node.id)
            .into_iter()
            .flatten()
            .filter(|_| flow.language != Language::Typescript)
            .map(|(name, info)| (name.as_str(), info.clone()))
            .collect();
        if let Some(expr) = &mut node.expr {
            update_reads(expr, &defaults);
        }
        let mut exact = HashMap::new();
        if let Some(expr) = &node.expr {
            exact_reads(expr, &mut exact);
        }
        for pin in node
            .inputs
            .iter_mut()
            .filter(|pin| pin.kind == PinKind::Data)
        {
            let Some(name) = pin.id.strip_prefix("use:") else {
                continue;
            };
            if let Some(types) = exact.get(name) {
                let first = types[0];
                pin.type_info = Some(if types.iter().all(|info| *info == first) {
                    first.clone()
                } else {
                    TypeInfo::new(
                        TypeShape::union(types.iter().map(|info| info.shape.clone())),
                        first.origin,
                        None,
                    )
                });
            } else if pin.type_info.is_none() {
                pin.type_info = defaults.get(name).cloned().flatten();
            }
        }
    }
}

type TypeState = Vec<Option<TypeInfo>>;

fn join_type(left: &Option<TypeInfo>, right: &Option<TypeInfo>) -> Option<TypeInfo> {
    match (left, right) {
        (Some(a), Some(b)) if a == b => Some(a.clone()),
        (Some(a), Some(b)) => Some(syntax_info(TypeShape::union([
            a.shape.clone(),
            b.shape.clone(),
        ]))),
        _ => None,
    }
}

fn merge_state(state: &mut TypeState, other: &TypeState) {
    for (left, right) in state.iter_mut().zip(other) {
        *left = join_type(left, right);
    }
}

#[derive(Default)]
struct ReachingTypes {
    sites: HashMap<Range, Option<TypeInfo>>,
    reads: HashMap<usize, Option<TypeInfo>>,
}

struct TypeEffects<'a, 'src> {
    src: Src<'src>,
    expressions: &'a HashMap<Range, TypeInfo>,
    reads: &'a HashMap<String, usize>,
    writes: &'a HashMap<String, usize>,
    declarations: &'a [Option<TypeInfo>],
    handled: HashSet<usize>,
    shadowed: Vec<&'src str>,
    observed: Option<&'a mut ReachingTypes>,
}

fn chain_link(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "member_expression" | "subscript_expression" | "call_expression"
    )
}

impl TypeEffects<'_, '_> {
    fn read(&mut self, node: Node<'_>, state: &TypeState) -> Option<TypeInfo> {
        let name = self.src.text(node);
        if self.shadowed.contains(&name) {
            return None;
        }
        let &slot = self.reads.get(name)?;
        let info = state[slot].clone();
        if let Some(observed) = &mut self.observed {
            let displayed = info
                .clone()
                .or_else(|| Some(syntax_info(TypeShape::Unknown)));
            observed.sites.insert(range(node), displayed.clone());
            observed
                .reads
                .entry(slot)
                .and_modify(|previous| *previous = join_type(previous, &displayed))
                .or_insert(displayed);
        }
        info
    }

    fn write(&mut self, target: Node<'_>, assigned: Option<TypeInfo>, state: &mut TypeState) {
        let target = value_operand(target);
        if target.kind() != "identifier" {
            return;
        }
        let name = self.src.text(target);
        if !self.shadowed.contains(&name)
            && let Some(&slot) = self.writes.get(name)
        {
            state[slot] = self.declarations[slot].clone().or(assigned);
            self.handled.insert(slot);
            if let Some(observed) = &mut self.observed {
                observed.sites.insert(range(target), state[slot].clone());
            }
        }
    }

    /// A parenthesized receiver starts a different chain. A continuous chain only merges
    /// its skipped paths after evaluating the last link.
    fn chain(&mut self, mut node: Node<'_>, state: &mut TypeState) -> Option<TypeInfo> {
        let mut links = Vec::new();
        loop {
            links.push(node);
            let receiver_field = if node.kind() == "call_expression" {
                "function"
            } else {
                "object"
            };
            let Some(receiver) = node.child_by_field_name(receiver_field) else {
                break;
            };
            if chain_link(receiver) {
                node = receiver;
            } else {
                self.evaluate(receiver, state);
                break;
            }
        }
        let mut skipped: Option<TypeState> = None;
        let mut result = None;
        for link in links.into_iter().rev() {
            if crate::syntax::children(link)
                .iter()
                .any(|child| matches!(child.kind(), "optional_chain" | "?."))
            {
                if let Some(skipped) = &mut skipped {
                    merge_state(skipped, state);
                } else {
                    skipped = Some(state.clone());
                }
            }
            if let Some(operand) = link
                .child_by_field_name("index")
                .or_else(|| link.child_by_field_name("arguments"))
            {
                self.evaluate(operand, state);
            }
            result = if let Some(function) = immediately_called_script_function(link) {
                self.immediate_call(function, state)
            } else {
                self.expressions.get(&range(link)).cloned()
            };
        }
        if let Some(skipped) = skipped {
            merge_state(state, &skipped);
            result = join_type(&result, &Some(syntax_info(TypeShape::scalar("Undefined"))));
        }
        result
    }

    fn immediate_call(&mut self, function: Node<'_>, state: &mut TypeState) -> Option<TypeInfo> {
        let body = function.child_by_field_name("body")?;
        let block = body.kind() == "statement_block";
        let statements: Vec<_> = named_children(body)
            .into_iter()
            .filter(|node| !node.is_extra())
            .collect();
        let simple = !crate::syntax::children(function)
            .iter()
            .any(|node| node.kind() == "async")
            && script_parameters(function).iter().all(|parameter| {
                parameter.kind() == "identifier"
                    || parameter
                        .child_by_field_name("pattern")
                        .is_some_and(|pattern| pattern.kind() == "identifier")
                        && parameter.child_by_field_name("value").is_none()
            })
            && (!block
                || statements.iter().all(|stmt| {
                    matches!(
                        stmt.kind(),
                        "expression_statement" | "empty_statement" | "return_statement"
                    )
                }));
        if !simple {
            for name in immediate_script_writes(self.src, function) {
                if !self.shadowed.contains(&name)
                    && let Some(&slot) = self.writes.get(name)
                {
                    state[slot] = self.declarations[slot].clone();
                    self.handled.insert(slot);
                }
            }
            return None;
        }
        let outer = self.shadowed.len();
        script_function_locals(self.src, function, &mut self.shadowed);
        let result = if block {
            let mut result = None;
            for stmt in statements {
                result = self.evaluate(stmt, state);
                if stmt.kind() == "return_statement" {
                    break;
                }
                result = None;
            }
            result
        } else {
            self.evaluate(body, state)
        };
        self.shadowed.truncate(outer);
        result
    }

    fn evaluate(&mut self, node: Node<'_>, state: &mut TypeState) -> Option<TypeInfo> {
        let node = value_operand(node);
        if node.is_extra() || script_function_boundary(node) {
            return self.expressions.get(&range(node)).cloned();
        }
        if chain_link(node) {
            return self.chain(node, state);
        }
        match node.kind() {
            "identifier" | "shorthand_property_identifier" => self.read(node, state),
            "type_annotation" | "type_arguments" | "type_parameters" => None,
            "ternary_expression" => {
                self.evaluate(node.child_by_field_name("condition")?, state);
                let mut other = state.clone();
                let yes = self.evaluate(node.child_by_field_name("consequence")?, state);
                let no = self.evaluate(node.child_by_field_name("alternative")?, &mut other);
                merge_state(state, &other);
                join_type(&yes, &no)
            }
            "assignment_expression" | "augmented_assignment_expression" | "binary_expression" => {
                let left = node.child_by_field_name("left")?;
                let right = node.child_by_field_name("right")?;
                let simple = node.kind() == "assignment_expression";
                let left_type = if simple && value_operand(left).kind() == "identifier" {
                    None
                } else {
                    self.evaluate(left, state)
                };
                let op = node
                    .child_by_field_name("operator")
                    .map(|op| self.src.text(op));
                let conditional = matches!(op, Some("&&" | "||" | "??" | "&&=" | "||=" | "??="));
                let before = conditional.then(|| state.clone());
                let rhs = self.evaluate(right, state);
                if node.kind() != "binary_expression" {
                    self.write(left, if simple { rhs.clone() } else { None }, state);
                }
                if let Some(before) = before {
                    merge_state(state, &before);
                }
                let result = if simple {
                    rhs
                } else if conditional {
                    join_type(&left_type, &rhs)
                } else {
                    None
                };
                if simple && let Some(observed) = &mut self.observed {
                    observed.sites.insert(range(node), result.clone());
                }
                result
            }
            "update_expression" => {
                let target = node.child_by_field_name("argument")?;
                self.evaluate(target, state);
                self.write(target, None, state);
                None
            }
            _ => {
                let mut result = None;
                for child in named_children(node)
                    .into_iter()
                    .filter(|child| !child.is_extra())
                {
                    result = self.evaluate(child, state);
                }
                match node.kind() {
                    "return_statement" | "expression_statement" | "sequence_expression" => result,
                    _ => self.expressions.get(&range(node)).cloned(),
                }
            }
        }
    }

    fn transfer(
        &mut self,
        node: &nodex_ir::FlowNode,
        expression: Option<Node<'_>>,
        switch: Option<Node<'_>>,
        state: &mut TypeState,
    ) -> Option<TypeInfo> {
        let value = expression.map_or_else(
            || node.expr.as_ref().and_then(|expr| expr.type_info.clone()),
            |expression| self.evaluate(expression, state),
        );
        if let Some(body) = switch.and_then(|node| node.child_by_field_name("body")) {
            let mut earlier_matches: Option<TypeState> = None;
            for case in named_children(body)
                .into_iter()
                .filter(|node| node.kind() == "switch_case")
            {
                if let Some(test) = case.child_by_field_name("value") {
                    self.evaluate(test, state);
                    if let Some(earlier) = &mut earlier_matches {
                        merge_state(earlier, state);
                    } else {
                        earlier_matches = Some(state.clone());
                    }
                }
            }
            if let Some(earlier) = earlier_matches {
                merge_state(state, &earlier);
            }
        }
        for (name, &slot) in self.writes {
            if !self.handled.contains(&slot) {
                let redeclaration = node.concept == Concept::Var
                    && node
                        .outputs
                        .iter()
                        .any(|pin| pin.id.strip_prefix("write:") == Some(name.as_str()));
                state[slot] = self.declarations[slot]
                    .clone()
                    .or_else(|| redeclaration.then(|| value.clone()).flatten());
            }
        }
        value.map(|info| TypeInfo::new(info.shape, TypeOrigin::Syntax, info.native))
    }
}

fn apply_reaching_types(expr: &mut Expr, sites: &HashMap<Range, Option<TypeInfo>>) {
    if let Some(info) = sites.get(&(expr.span.start_byte, expr.span.end_byte)) {
        expr.type_info = info.clone();
    }
    for child in &mut expr.children {
        apply_reaching_types(child, sites);
    }
}

fn track_assignments<'tree>(
    flow: &mut FlowGraph,
    root: Node<'tree>,
    src: Src<'_>,
    expressions: &HashMap<Range, TypeInfo>,
    continuations: Option<&Continuations>,
    compound: &HashMap<u32, Node<'tree>>,
) {
    let positions: HashMap<_, _> = flow
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();
    let mut slots = HashMap::new();
    let mut declared_types = Vec::new();
    let mut definitions = vec![Vec::new(); flow.nodes.len()];
    for (i, node) in flow.nodes.iter().enumerate() {
        for pin in node.outputs.iter().filter(|pin| pin.id.starts_with("var:")) {
            let slot = slots.len();
            slots.insert((node.id.as_str(), pin.id.as_str()), slot);
            let info = pin.type_info.clone();
            declared_types.push(
                info.clone()
                    .filter(|info| info.origin == TypeOrigin::Declared),
            );
            definitions[i].push((slot, info));
        }
    }
    if flow.nodes.is_empty() || flow.nodes.len().saturating_mul(slots.len()) > 200_000 {
        return;
    }
    let syntax: Vec<_> = flow
        .nodes
        .iter()
        .map(|node| {
            let expression = node.expr.as_ref().and_then(|expr| {
                let start = expr.span.start_byte as usize;
                let end = expr.span.end_byte as usize;
                root.descendant_for_byte_range(start, end)
                    .filter(|site| site.start_byte() == start && site.end_byte() == end)
            });
            let switch = compound
                .get(&node.span.start_byte)
                .copied()
                .filter(|site| site.kind() == "switch_statement");
            (expression, switch)
        })
        .collect();
    // These visual continuations omit some execution paths, such as a break inside a loop.
    // Forget the changed bindings at that boundary instead of showing a stale reaching type.
    let uncertain: HashMap<_, Vec<_>> = flow
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(i, node)| {
            let &statement = compound.get(&node.span.start_byte)?;
            let end = statement.end_byte() as u32;
            let changed = flow
                .nodes
                .iter()
                .filter(|child| {
                    child.span.start_byte >= node.span.start_byte
                        && child.span.end_byte <= end
                        && (statement.kind() != "switch_statement" || child.id != node.id)
                })
                .flat_map(|child| &child.binding_writes)
                .filter_map(|write| {
                    slots
                        .get(&(write.node.as_str(), write.pin.as_str()))
                        .copied()
                })
                .collect();
            Some((i, changed))
        })
        .collect();
    let mut reads = vec![HashMap::new(); flow.nodes.len()];
    let mut successors = vec![Vec::new(); flow.nodes.len()];
    for edge in &flow.edges {
        let Some(&to) = positions.get(edge.to.as_str()) else {
            continue;
        };
        if edge.kind == FlowEdgeKind::Exec {
            let key = (edge.from.clone(), edge.from_pin.clone());
            let mut exits = vec![&key];
            while let Some(exit) = exits.pop() {
                if let Some(replacements) = continuations.and_then(|paths| paths.get(exit)) {
                    exits.extend(replacements);
                } else if let Some(&position) = positions.get(exit.0.as_str()) {
                    successors[position].push(to);
                }
            }
        } else if let Some(name) = edge.to_pin.strip_prefix("use:")
            && let Some(&slot) = slots.get(&(edge.from.as_str(), edge.from_pin.as_str()))
        {
            reads[to].insert(name.to_owned(), slot);
        }
    }
    let writes: Vec<HashMap<_, _>> = flow
        .nodes
        .iter()
        .map(|node| {
            node.binding_writes
                .iter()
                .filter_map(|binding| {
                    let name = binding.pin.strip_prefix("var:")?;
                    let &slot = slots.get(&(binding.node.as_str(), binding.pin.as_str()))?;
                    Some((name.to_owned(), slot))
                })
                .collect()
        })
        .collect();
    let effects = |i, observed| TypeEffects {
        src,
        expressions,
        reads: &reads[i],
        writes: &writes[i],
        declarations: &declared_types,
        handled: HashSet::new(),
        shadowed: Vec::new(),
        observed,
    };
    let forget_uncertain = |i, state: &mut TypeState| {
        if let Some(changed) = uncertain.get(&i) {
            for &slot in changed {
                state[slot] = declared_types[slot].clone();
            }
        }
    };
    let mut states: Vec<Option<TypeState>> = vec![None; flow.nodes.len()];
    states[0] = Some(vec![None; slots.len()]);
    let mut work = VecDeque::from([0]);
    let mut queued = vec![false; flow.nodes.len()];
    let mut iterations = 0;
    while let Some(i) = work.pop_front() {
        queued[i] = false;
        iterations += 1;
        if iterations > flow.nodes.len().saturating_mul(32) {
            return;
        }
        let mut next = states[i].as_ref().unwrap().clone();
        forget_uncertain(i, &mut next);
        let node = &flow.nodes[i];
        let value = effects(i, None).transfer(node, syntax[i].0, syntax[i].1, &mut next);
        for (slot, info) in &definitions[i] {
            next[*slot] = info.clone().or_else(|| {
                (node.concept == Concept::Var && definitions[i].len() == 1)
                    .then(|| value.clone())
                    .flatten()
            });
        }
        forget_uncertain(i, &mut next);
        for &to in &successors[i] {
            let changed = if let Some(previous) = &mut states[to] {
                let mut changed = false;
                for (left, right) in previous.iter_mut().zip(&next) {
                    let merged = join_type(left, right);
                    if *left != merged {
                        *left = merged;
                        changed = true;
                    }
                }
                changed
            } else {
                states[to] = Some(next.clone());
                true
            };
            if changed && !queued[to] {
                queued[to] = true;
                work.push_back(to);
            }
        }
    }
    for (i, node) in flow.nodes.iter_mut().enumerate() {
        let Some(state) = &states[i] else { continue };
        let mut before = state.clone();
        forget_uncertain(i, &mut before);
        let mut after = before.clone();
        let mut observed = ReachingTypes::default();
        let value = TypeEffects {
            src,
            expressions,
            reads: &reads[i],
            writes: &writes[i],
            declarations: &declared_types,
            handled: HashSet::new(),
            shadowed: Vec::new(),
            observed: Some(&mut observed),
        }
        .transfer(node, syntax[i].0, syntax[i].1, &mut after);
        for pin in &mut node.outputs {
            if let Some(name) = pin.id.strip_prefix("write:")
                && let Some(&slot) = writes[i].get(name)
            {
                pin.type_info = after[slot].clone();
            } else if pin.id.starts_with("var:")
                && pin.type_info.is_none()
                && node.concept == Concept::Var
                && definitions[i].len() == 1
            {
                pin.type_info = value.clone();
            }
        }
        for pin in &mut node.inputs {
            if let Some(&slot) = pin
                .id
                .strip_prefix("use:")
                .and_then(|name| reads[i].get(name))
            {
                pin.type_info = observed.reads.get(&slot).unwrap_or(&before[slot]).clone();
            }
        }
        if let Some(expr) = &mut node.expr {
            apply_reaching_types(expr, &observed.sites);
        }
    }
}
