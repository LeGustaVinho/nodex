use crate::syntax::{Src, children, named_children};
use nodex_ir::{
    BindingForm, Concept, Expr, FileIr, Language, PassingMode, PinKind, ValueBasis, ValueInfo,
    ValueKind, ValueQualifier,
};
use std::collections::HashMap;
use tree_sitter::Node;

pub fn annotate(ir: &mut FileIr, root: Node, src: Src) {
    let mut expressions = HashMap::new();
    let mut bindings = Vec::new();
    let mut nodes = vec![root];
    while let Some(n) = nodes.pop() {
        if n.is_error() || n.is_missing() || n.is_extra() || n.kind() == "token_tree" {
            continue;
        }
        nodes.extend(children(n).into_iter().rev());
        if let Some(info) = expression_shape(ir.language, n) {
            expressions.insert((n.start_byte() as u32, n.end_byte() as u32), info);
        }
        if let Some((name, ty, value, parameter)) = declaration(n) {
            let type_text = ty.map(|t| src.norm(t));
            let mut info = type_text
                .as_deref()
                .and_then(|t| written_type(ir.language, t));
            if ir.language == Language::Typescript && info.is_none() {
                info = ty.and_then(typescript_type);
            }
            let may_infer = match ir.language {
                Language::Typescript => n.parent().is_some_and(|p| {
                    p.kind() == "lexical_declaration"
                        && children(p).iter().any(|c| c.kind() == "const")
                }),
                Language::Csharp | Language::Rust => true,
            };
            if info.is_none() && may_infer && (ty.is_none() || type_text.as_deref() == Some("var"))
            {
                info = value.and_then(|v| expression_shape(ir.language, v));
            }
            if ir.language == Language::Csharp {
                if let Some(passing) = csharp_mode(src, n, ty) {
                    info = Some(shape(ValueKind::Reference, Some(ValueQualifier::Alias)));
                    info.as_mut().unwrap().passing = Some(passing);
                } else if parameter {
                    let info = info.get_or_insert_with(|| shape(ValueKind::Unknown, None));
                    info.passing = Some(PassingMode::ByValue);
                }
            } else if parameter && ir.language == Language::Typescript {
                info.get_or_insert_with(|| shape(ValueKind::Unknown, None))
                    .passing = Some(PassingMode::ByValue);
            }
            bindings.push((src.span(name), src.text(name), info));
        }
    }
    bindings.sort_by_key(|(span, _, _)| span.start_byte);
    for symbol in &mut ir.symbols {
        if let Some(ty) = &symbol.type_text {
            symbol.value_info = written_type(ir.language, ty);
        }
    }

    for flow in &mut ir.flows {
        for node in &mut flow.nodes {
            let first =
                bindings.partition_point(|(span, _, _)| span.start_byte < node.span.start_byte);
            let candidates = &bindings[first..];
            for pin in node.outputs.iter_mut().filter(|p| p.kind == PinKind::Data) {
                if let Some(name) = pin.id.strip_prefix("var:") {
                    let mut matching = candidates
                        .iter()
                        .take_while(|(span, _, _)| span.start_byte < node.span.end_byte)
                        .filter(|(span, candidate, _)| {
                            *candidate == name && span.end_byte <= node.span.end_byte
                        });
                    if let Some((span, _, info)) =
                        matching.next().filter(|_| matching.next().is_none())
                    {
                        pin.declaration = Some(span.clone());
                        pin.value_info = *info;
                    }
                }
                pin.value_info = pin.value_info.or_else(|| {
                    pin.ty
                        .as_deref()
                        .and_then(|ty| written_type(ir.language, ty))
                });
                if pin
                    .binding
                    .as_ref()
                    .is_some_and(|binding| binding.form == BindingForm::RefReadonly)
                {
                    let mut info = shape(ValueKind::Reference, Some(ValueQualifier::Alias));
                    info.passing = Some(PassingMode::RefReadonly);
                    pin.value_info = Some(info);
                }
            }
            if let Some(expr) = &mut node.expr {
                annotate_expr(ir.language, expr, &expressions);
            }
        }
        carry_to_uses(flow);
    }
}

pub(crate) fn declaration(n: Node<'_>) -> Option<(Node<'_>, Option<Node<'_>>, Option<Node<'_>>, bool)> {
    let kind = n.kind();
    if kind == "self_parameter" {
        let name = children(n).into_iter().find(|c| c.kind() == "self")?;
        return Some((name, Some(n), None, true));
    }
    if !matches!(
        kind,
        "variable_declarator"
            | "let_declaration"
            | "parameter"
            | "required_parameter"
            | "optional_parameter"
            | "for_each_statement"
            | "declaration_expression"
            | "single_variable_designation"
            | "public_field_definition"
            | "property_signature"
            | "property_declaration"
            | "field_declaration"
    ) {
        return None;
    }
    let name = n
        .child_by_field_name("name")
        .or_else(|| n.child_by_field_name("pattern"))
        .or_else(|| n.child_by_field_name("left"))
        .or_else(|| {
            (kind == "variable_declarator")
                .then(|| named_children(n).into_iter().next())
                .flatten()
        })?;
    if !matches!(
        name.kind(),
        "identifier" | "self" | "private_property_identifier"
    ) {
        return None;
    }
    let ty = n.child_by_field_name("type").or_else(|| {
        n.parent()
            .filter(|p| p.kind() == "variable_declaration" && kind == "variable_declarator")
            .and_then(|p| p.child_by_field_name("type"))
    });
    let value = n.child_by_field_name("value").or_else(|| {
        (kind == "variable_declarator")
            .then(|| {
                named_children(n)
                    .into_iter()
                    .find(|c| c.start_byte() >= name.end_byte())
            })
            .flatten()
    });
    Some((name, ty, value, kind.ends_with("parameter")))
}

fn typescript_type(n: Node) -> Option<ValueInfo> {
    match n.kind() {
        "type_annotation" | "parenthesized_type" | "readonly_type" => named_children(n)
            .into_iter()
            .next()
            .and_then(typescript_type),
        "array_type" | "tuple_type" | "function_type" | "constructor_type" => {
            Some(shape(ValueKind::Reference, Some(ValueQualifier::Object)))
        }
        "literal_type" => named_children(n)
            .into_iter()
            .next()
            .and_then(|n| expression_shape(Language::Typescript, n)),
        _ => None,
    }
}

fn csharp_mode(src: Src, n: Node, ty: Option<Node>) -> Option<PassingMode> {
    let modifiers: Vec<_> = children(n)
        .into_iter()
        .filter(|c| !c.is_extra() && (!c.is_named() || c.kind() == "modifier"))
        .map(|c| src.text(c))
        .collect();
    let readonly = modifiers.contains(&"readonly") || modifiers.contains(&"ref readonly");
    if modifiers.contains(&"ref")
        || modifiers.contains(&"ref readonly")
        || ty.is_some_and(|t| t.kind() == "ref_type")
    {
        Some(
            if readonly
                || ty.is_some_and(|t| src.text(t).split_whitespace().nth(1) == Some("readonly"))
            {
                PassingMode::RefReadonly
            } else {
                PassingMode::Ref
            },
        )
    } else if modifiers.contains(&"in") {
        Some(PassingMode::In)
    } else if modifiers.contains(&"out") {
        Some(PassingMode::Out)
    } else {
        None
    }
}

fn expression_shape(language: Language, n: Node) -> Option<ValueInfo> {
    use ValueKind::*;
    let kind = n.kind();
    let value = || shape(Value, None);
    let object = || shape(Reference, Some(ValueQualifier::Object));
    let raw = || shape(Pointer, Some(ValueQualifier::Raw));
    match language {
        Language::Csharp => match kind {
            "integer_literal" | "real_literal" | "boolean_literal" | "character_literal"
            | "tuple_expression" => Some(value()),
            "string_literal"
            | "verbatim_string_literal"
            | "raw_string_literal"
            | "interpolated_string_expression"
            | "array_creation_expression"
            | "implicit_array_creation_expression"
            | "anonymous_object_creation_expression" => Some(object()),
            _ => None,
        },
        Language::Typescript => match kind {
            "number" | "string" | "template_string" | "true" | "false" | "null" => Some(value()),
            "object"
            | "array"
            | "regex"
            | "arrow_function"
            | "function_expression"
            | "generator_function"
            | "new_expression"
            | "class" => Some(object()),
            _ => None,
        },
        Language::Rust => match kind {
            "integer_literal" | "float_literal" | "boolean_literal" | "char_literal"
            | "tuple_expression" | "array_expression" | "struct_expression" | "unit_expression"
            | "range_expression" => Some(value()),
            "reference_expression" => Some(if children(n).iter().any(|c| c.kind() == "raw") {
                raw()
            } else {
                borrow_shape(children(n).iter().any(|c| c.kind() == "mutable_specifier"))
            }),
            "string_literal" | "raw_string_literal" => Some(borrow_shape(false)),
            _ => None,
        },
    }
}

fn annotate_expr(language: Language, e: &mut Expr, shapes: &HashMap<(u32, u32), ValueInfo>) {
    e.value_info = shapes.get(&(e.span.start_byte, e.span.end_byte)).copied();
    if language == Language::Rust {
        // A dereference yields the underlying place, not the shape of the wrapped operand.
        if let Some(site) = e
            .syntax_sites
            .iter()
            .flatten()
            .filter(|s| {
                matches!(
                    s.key.as_str(),
                    "borrow" | "borrow-mut" | "raw-borrow" | "deref" | "try"
                )
            })
            .min_by_key(|s| s.span.start_byte)
        {
            e.value_info = match site.key.as_str() {
                "borrow" => Some(borrow_shape(false)),
                "borrow-mut" => Some(borrow_shape(true)),
                "raw-borrow" => Some(shape(ValueKind::Pointer, Some(ValueQualifier::Raw))),
                _ => None,
            };
        }
    }
    if let Some(mode) = e.flavors.iter().find_map(|f| match f.as_str() {
        "argument:ref" => Some(PassingMode::Ref),
        "argument:in" => Some(PassingMode::In),
        "argument:out" => Some(PassingMode::Out),
        _ => None,
    }) {
        e.value_info
            .get_or_insert_with(|| shape(ValueKind::Unknown, None))
            .passing = Some(mode);
    }
    for child in &mut e.children {
        annotate_expr(language, child, shapes);
    }
}

fn shape(kind: ValueKind, qualifier: Option<ValueQualifier>) -> ValueInfo {
    ValueInfo {
        kind,
        basis: ValueBasis::Syntax,
        qualifier,
        passing: None,
    }
}

fn borrow_shape(mutable: bool) -> ValueInfo {
    shape(
        ValueKind::Reference,
        Some(if mutable {
            ValueQualifier::ExclusiveBorrow
        } else {
            ValueQualifier::SharedBorrow
        }),
    )
}

/// Classify type forms with an unambiguous outer shape. Unresolved named types stay unknown.
pub fn written_type(language: Language, ty: &str) -> Option<ValueInfo> {
    use ValueKind::*;
    let t = ty.trim().strip_prefix(':').unwrap_or(ty.trim()).trim();
    match language {
        Language::Csharp => {
            if let Some(referent) = t.strip_prefix("ref ") {
                let mut info = shape(Reference, Some(ValueQualifier::Alias));
                info.passing = Some(if referent.starts_with("readonly ") {
                    PassingMode::RefReadonly
                } else {
                    PassingMode::Ref
                });
                return Some(info);
            }
            if t.trim_end_matches('?').ends_with(']')
                || matches!(t.trim_end_matches('?'), "string" | "object" | "dynamic")
            {
                return Some(shape(Reference, Some(ValueQualifier::Object)));
            }
            if t.ends_with('*') || t.starts_with("delegate*") {
                return Some(shape(Pointer, Some(ValueQualifier::Raw)));
            }
            matches!(
                t.trim_end_matches('?'),
                "bool"
                    | "byte"
                    | "sbyte"
                    | "short"
                    | "ushort"
                    | "int"
                    | "uint"
                    | "long"
                    | "ulong"
                    | "float"
                    | "double"
                    | "decimal"
                    | "char"
                    | "nint"
                    | "nuint"
            )
            .then(|| shape(Value, None))
        }
        Language::Typescript => match t {
            "number" | "string" | "boolean" | "bigint" | "symbol" | "null" | "undefined" => {
                Some(shape(Value, None))
            }
            "object" => Some(shape(Reference, Some(ValueQualifier::Object))),
            _ => None,
        },
        Language::Rust => {
            let t = ungroup_rust_type(t);
            if let Some(rest) = t.strip_prefix('&') {
                let mut words = rest.split_whitespace();
                let first = words.next().unwrap_or("");
                let modifier = if first.starts_with('\'') {
                    words.next().unwrap_or("")
                } else {
                    first
                };
                return Some(borrow_shape(modifier == "mut"));
            }
            if t.starts_with("*const ") || t.starts_with("*mut ") || t.starts_with("fn(") {
                return Some(shape(Pointer, Some(ValueQualifier::Raw)));
            }
            let outer = t.trim_start_matches("::").split('<').next().unwrap_or(t);
            let qualifier = match outer {
                "std::boxed::Box" | "alloc::boxed::Box" => Some(ValueQualifier::Unique),
                "std::rc::Rc" | "std::sync::Arc" | "alloc::rc::Rc" | "alloc::sync::Arc" => {
                    Some(ValueQualifier::Shared)
                }
                "std::rc::Weak" | "std::sync::Weak" | "alloc::rc::Weak" | "alloc::sync::Weak" => {
                    Some(ValueQualifier::Weak)
                }
                _ => None,
            };
            if matches!(
                outer,
                "std::string::String"
                    | "alloc::string::String"
                    | "std::vec::Vec"
                    | "alloc::vec::Vec"
                    | "std::option::Option"
                    | "core::option::Option"
                    | "std::result::Result"
                    | "core::result::Result"
            ) {
                return Some(shape(Value, None));
            }
            if qualifier.is_some() {
                return Some(shape(Pointer, qualifier));
            }
            (t.starts_with('(')
                || t.starts_with('[')
                || matches!(
                    t,
                    "i8" | "i16"
                        | "i32"
                        | "i64"
                        | "i128"
                        | "isize"
                        | "u8"
                        | "u16"
                        | "u32"
                        | "u64"
                        | "u128"
                        | "usize"
                        | "f32"
                        | "f64"
                        | "bool"
                        | "char"
                        | "{integer}"
                        | "{float}"
                ))
            .then(|| shape(Value, None))
        }
    }
}

fn ungroup_rust_type(mut ty: &str) -> &str {
    while let Some(inner) = ty.strip_prefix('(').and_then(|rest| rest.strip_suffix(')')) {
        let mut depth = 0;
        let grouped = !inner.trim().is_empty()
            && inner.char_indices().all(|(i, ch)| match ch {
                '>' if inner.as_bytes().get(i.wrapping_sub(1)) == Some(&b'-') => true,
                '(' | '[' | '<' | '{' => {
                    depth += 1;
                    true
                }
                ')' | ']' | '>' | '}' if depth > 0 => {
                    depth -= 1;
                    true
                }
                ')' | ',' => depth != 0,
                _ => true,
            });
        if !grouped || depth != 0 {
            break;
        }
        ty = inner.trim();
    }
    ty
}

pub fn carry_to_uses(flow: &mut nodex_ir::FlowGraph) {
    let definitions: HashMap<_, _> = flow
        .nodes
        .iter()
        .flat_map(|node| {
            node.outputs.iter().filter_map(|pin| {
                pin.value_info
                    .map(|info| ((node.id.as_str(), pin.id.as_str()), info))
            })
        })
        .collect();
    let mut uses = HashMap::new();
    for edge in &flow.edges {
        if let Some(info) = definitions.get(&(edge.from.as_str(), edge.from_pin.as_str())) {
            let mut info = *info;
            info.passing = None;
            uses.insert((edge.to.clone(), edge.to_pin.clone()), info);
        }
    }
    for node in &mut flow.nodes {
        let mut read_shapes = HashMap::new();
        if flow.language == Language::Typescript
            && let Some(expr) = &node.expr
        {
            collect_read_shapes(expr, &mut read_shapes);
        }
        let mut values = HashMap::new();
        for pin in &mut node.inputs {
            let definition = uses.get(&(node.id.clone(), pin.id.clone())).copied();
            let read = pin.id.strip_prefix("use:").and_then(|name| {
                if let Some(info) = definition {
                    values.insert(name, info);
                }
                read_shapes.get(name).copied().flatten()
            });
            pin.value_info = read.or(definition).or(pin.value_info);
        }
        if let Some(expr) = &mut node.expr {
            fill_reads(expr, &values);
        }
    }
}

fn collect_read_shapes<'a>(expr: &'a Expr, shapes: &mut HashMap<&'a str, Option<ValueInfo>>) {
    if expr.concept == Concept::Lambda {
        return;
    }
    if expr.concept == Concept::Var {
        let shape = expr
            .value_info
            .filter(|info| {
                info.basis == ValueBasis::TypeAnalysis && info.kind != ValueKind::Unknown
            })
            .map(|info| ValueInfo {
                passing: None,
                ..info
            });
        // One statement pin can stand for several uses. Refine it only if they agree.
        shapes
            .entry(&expr.label)
            .and_modify(|previous| {
                if *previous != shape {
                    *previous = None;
                }
            })
            .or_insert(shape);
    }
    for child in &expr.children {
        collect_read_shapes(child, shapes);
    }
}

fn fill_reads(expr: &mut Expr, values: &HashMap<&str, ValueInfo>) {
    if expr.concept == Concept::Lambda {
        return;
    }
    if expr.concept == Concept::Var
        && !expr
            .syntax_sites
            .iter()
            .flatten()
            .any(|site| matches!(site.key.as_str(), "deref" | "try"))
        && expr
            .value_info
            .is_none_or(|info| info.kind == ValueKind::Unknown && info.basis == ValueBasis::Syntax)
        && let Some(info) = values.get(expr.label.as_str())
    {
        expr.value_info = Some(ValueInfo {
            passing: expr.value_info.and_then(|previous| previous.passing),
            ..*info
        });
    }
    for child in &mut expr.children {
        fill_reads(child, values);
    }
}
