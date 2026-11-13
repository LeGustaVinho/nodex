use std::collections::{BTreeMap, HashMap, HashSet};

use nodex_ir::{
    Access, Concept, Language, Span, ValueBasis,
    borrow::{BorrowFact, BorrowInput, BorrowRule},
    reading::{ContractItem, ReadingBranch, ReadingContent, ReadingFact, ReadingPart, ReadingStep},
    semantic::{ByteRange, TypeFact, ValueFactTarget},
    types::{CollectionKind, TypeInfo, TypeOrigin, TypeShape, syntax_shape},
};
use nodex_lang_api::{
    blueprint_types::{expression_type, project_pattern, rust_grouped_pattern},
    kit::truncate,
    syntax::{Src, children, named_children},
};
use tree_sitter::Node;

use crate::{expr::method_call, syntax::{block_statements, inclusive_range_to, statement_tail, unparen}};

type Range = (usize, usize);

pub(crate) fn contract(src: Src, node: Node) -> Option<Vec<ContractItem>> {
    let function = matches!(node.kind(), "function_item" | "function_signature_item");
    let member_owner = node.parent().and_then(|body| body.parent());
    let owner = member_owner.filter(|owner| owner.kind() == "impl_item");
    let mut items = Vec::new();
    let mut add = |at, group: &str, text: String, ty: Option<Node>| {
        let type_info = ty.map(|ty| {
            let native = src.norm(ty);
            TypeInfo::new(syntax_shape(Language::Rust, &native), TypeOrigin::Declared, Some(native))
        });
        items.push(ContractItem { group: group.into(), text, span: src.span(at), type_info });
    };
    if let Some(name) = node.child_by_field_name("name") {
        let name = || {
            let mut written = src.norm(name);
            if let Some(parameters) = node.child_by_field_name("type_parameters") {
                written.push_str(&src.norm(parameters));
            }
            written
        };
        match node.kind() {
            "associated_type" if member_owner.is_some_and(|owner| owner.kind() == "trait_item") => {
                add(node, "Associated type", format!("Each implementation chooses a type for {}.", name()), None);
                if let Some(bounds) = node.child_by_field_name("bounds") {
                    let bounds_text = src.norm(bounds);
                    let requirements = bounds_text.trim_start_matches(':').trim();
                    add(bounds, "Requires", format!("The chosen type must meet {requirements}."), None);
                }
            }
            "type_item" if owner.and_then(|owner| owner.child_by_field_name("trait")).is_some() => {
                if let Some(ty) = node.child_by_field_name("type") {
                    add(node, "Associated type", format!("This implementation sets {} to {}.", name(), src.norm(ty)), Some(ty));
                }
            }
            _ => {}
        }
    }
    if function {
        let name = node.child_by_field_name("name").unwrap_or(node);
        let params: Vec<_> = node.child_by_field_name("parameters").map(named_children).unwrap_or_default()
            .into_iter().filter(|param| !param.is_extra() && param.kind() != "attribute_item").collect();
        for param in &params {
            if param.kind() == "self_parameter" {
                let native = src.norm(*param);
                let (group, text) = if native.starts_with('&') && children(*param).iter().any(|child| child.kind() == "mutable_specifier") {
                    ("May change", "Borrow the receiver with permission to change it")
                } else if native.starts_with('&') {
                    ("Receives", "Borrow the receiver for reading")
                } else if native == "self" || native == "mut self" {
                    ("Receives", "Take the receiver by value")
                } else {
                    ("Receives", "Receiver passed as written")
                };
                add(*param, group, text.into(), Some(*param));
            } else if let Some(pattern) = param.child_by_field_name("pattern") {
                let ty = param.child_by_field_name("type");
                let group = if ty.is_some_and(|ty| ty.kind() == "reference_type"
                    && children(ty).iter().any(|child| child.kind() == "mutable_specifier")) { "May change" } else { "Receives" };
                let mut description = src.norm(pattern);
                if let Some(ty) = ty {
                    let inner = ty.child_by_field_name("type").unwrap_or(ty);
                    match inner.kind() {
                        "abstract_type" => description.push_str(" · accepts any concrete type meeting this contract"),
                        "dynamic_type" => description.push_str(" · calls through this trait contract at runtime"),
                        _ => {}
                    }
                }
                add(*param, group, description, ty);
            }
        }
        if let Some(result) = node.child_by_field_name("return_type") {
            let description = match result.kind() {
                "abstract_type" => "A concrete type chosen by this implementation that meets the listed requirements",
                "reference_type" => "A reference to data that must remain available for the required lifetime",
                _ => "Value returned to the caller",
            };
            add(result, "Returns", description.into(), Some(result));
            if let Some((at, text)) = reference_lifetime(src, result, &params) {
                add(at, "Lifetime", text, None);
            }
        } else {
            add(name, "Returns", "No explicit result (Unit)".into(), None);
        }
    }
    for declaration in owner.into_iter().chain(std::iter::once(node)) {
        if let Some(trait_) = declaration.child_by_field_name("trait") {
            add(trait_, "Implements", src.norm(trait_), declaration.child_by_field_name("type"));
        }
        for part in named_children(declaration) {
            match part.kind() {
                "type_parameters" | "where_clause" => {
                    for parameter in named_children(part).into_iter().filter(|parameter| !parameter.is_extra()) {
                        let bounds = parameter.child_by_field_name("bounds");
                        let name = parameter.child_by_field_name("name").or_else(|| parameter.child_by_field_name("left"));
                        if let (Some(name), Some(bounds)) = (name, bounds) {
                            let requirements = src.norm(bounds);
                            add(parameter, "Requires", format!("{} must meet {}", src.norm(name), requirements.trim_start_matches(':').trim()), None);
                        } else {
                            add(parameter, if part.kind() == "where_clause" { "Requires" } else { "Type parameters" }, src.norm(parameter), None);
                        }
                    }
                }
                _ => {}
            }
        }
        for (binding, trait_name, associated, ty) in associated_type_bindings(declaration) {
            let mut name = src.norm(associated);
            if let Some(arguments) = binding.child_by_field_name("type_arguments") {
                name.push_str(&src.norm(arguments));
            }
            add(binding, "Requires", format!("The {} trait's {name} type must be {}.", src.norm(trait_name), src.norm(ty)), Some(ty));
        }
        let mut previous = declaration.prev_named_sibling();
        while let Some(attribute) = previous.filter(|part| part.kind() == "attribute_item" || part.is_extra()) {
            let written = src.norm(attribute);
            if written.starts_with("#[cfg(") || written.starts_with("#[cfg_attr(") {
                let compact = written.replace(char::is_whitespace, "");
                let text = match compact.as_str() {
                    "#[cfg(any())]" => format!("{written} · always excluded by this condition"),
                    "#[cfg(all())]" => format!("{written} · this condition is always true"),
                    _ => format!("{written} · depends on the build configuration"),
                };
                add(attribute, "Build condition", text, None);
            } else if written.starts_with("#[derive(") {
                add(attribute, "Generated implementations", written, None);
            }
            previous = attribute.prev_named_sibling();
        }
    }
    (!items.is_empty()).then_some(items)
}

fn associated_type_bindings(declaration: Node<'_>) -> Vec<(Node<'_>, Node<'_>, Node<'_>, Node<'_>)> {
    if declaration.kind() == "source_file" {
        return Vec::new();
    }
    let mut bindings = Vec::new();
    let body = declaration.child_by_field_name("body");
    let mut pending: Vec<_> = named_children(declaration).into_iter().rev().filter(|part| Some(*part) != body).collect();
    while let Some(part) = pending.pop() {
        if matches!(part.kind(), "block" | "declaration_list" | "token_tree") || part.is_extra() {
            continue;
        }
        if part.kind() == "type_binding"
            && let Some(trait_name) = part.parent().and_then(|arguments| arguments.parent())
                .filter(|generic| generic.kind() == "generic_type")
                .and_then(|generic| generic.child_by_field_name("type"))
            && let Some(associated) = part.child_by_field_name("name")
            && let Some(ty) = part.child_by_field_name("type")
        {
            bindings.push((part, trait_name, associated, ty));
        }
        pending.extend(named_children(part).into_iter().rev());
    }
    bindings
}

fn reference_lifetime<'tree>(src: Src, result: Node<'tree>, params: &[Node<'tree>]) -> Option<(Node<'tree>, String)> {
    use crate::syntax::unparen_type;

    let result = unparen_type(result);
    if result.kind() != "reference_type" {
        return None;
    }
    let explicit = children(result).into_iter().find(|child| child.kind() == "lifetime");
    if let Some(lifetime) = explicit.filter(|lifetime| src.text(*lifetime) == "'static") {
        return Some((lifetime, "The data behind this reference must remain valid for the rest of the program.".into()));
    }
    let references: Vec<_> = params.iter().filter_map(|param| {
        if param.kind() == "self_parameter" {
            return children(*param).iter().any(|token| token.kind() == "&").then_some((*param, *param));
        }
        param.child_by_field_name("type").map(unparen_type)
            .filter(|ty| ty.kind() == "reference_type").map(|ty| (*param, ty))
    }).collect();
    let parameter_name = |parameter: Node| src.norm(parameter.child_by_field_name("pattern").unwrap_or(parameter));
    if let Some(lifetime) = explicit.filter(|lifetime| src.text(*lifetime) != "'_") {
        let written = src.text(lifetime);
        let inputs: Vec<_> = references.iter().filter(|(_, ty)| {
            children(*ty).into_iter().any(|child| child.kind() == "lifetime" && src.text(child) == written)
        }).map(|(parameter, _)| parameter_name(*parameter)).collect();
        let text = if inputs.is_empty() {
            format!("The returned reference uses lifetime {written}.")
        } else {
            format!("The returned reference uses lifetime {written}, which also appears in {}.", inputs.join(", "))
        };
        return Some((lifetime, text));
    }

    // A borrowed receiver supplies the output lifetime even if other inputs also contain borrows.
    let receiver = references.iter().find(|(param, ty)| {
        param.kind() == "self_parameter"
            || param.child_by_field_name("pattern").is_some_and(|pattern| pattern.kind() == "self")
                && ty.child_by_field_name("type").is_some_and(|target| src.text(unparen_type(target)) == "Self")
    });
    let (parameter, _) = receiver.or_else(|| {
        let [reference @ (parameter, ty)] = references.as_slice() else { return None };
        let inner = ty.child_by_field_name("type")?;
        let other_types_are_lifetime_free = params.iter().filter(|other| *other != parameter)
            .all(|param| param.child_by_field_name("type").is_some_and(lifetime_free_type));
        (lifetime_free_type(inner) && other_types_are_lifetime_free).then_some(reference)
    })?;
    Some((explicit.unwrap_or(result), format!("The returned reference uses the same lifetime as the reference in {}.", parameter_name(*parameter))))
}

/// Recognize only written types whose structure proves they contain no lifetime parameters.
fn lifetime_free_type(ty: Node<'_>) -> bool {
    match ty.kind() {
        "primitive_type" | "unit_type" | "never_type" => true,
        "tuple_type" => named_children(ty).into_iter().filter(|part| !part.is_extra()).all(lifetime_free_type),
        "array_type" => ty.child_by_field_name("element").is_some_and(lifetime_free_type),
        "pointer_type" => ty.child_by_field_name("type").is_some_and(lifetime_free_type),
        _ => false,
    }
}

fn range(node: Node) -> Range {
    (node.start_byte(), node.end_byte())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Family {
    Bool,
    Option,
    Result,
    Map,
    Slice,
    Iter,
    ParIter,
    Thread,
    Task,
    Arc,
    Lock,
    Cell,
    Clone,
    Macro,
    Effect,
    ArrayFromFn,
    IterFromFn,
    MemReplace,
    NumericFrom { source: &'static str, target: &'static str },
    NumericTryFrom { source: &'static str, target: &'static str },
}

pub(crate) struct Resolved {
    pub family: Family,
    pub name: String,
    pub definition: Option<Span>,
}

pub(crate) type Operations = HashMap<Range, Option<Resolved>>;
pub(crate) type TypeIdentities = BTreeMap<usize, (usize, String)>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NumericAlias {
    pub primitive: &'static str,
    pub spelling: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DefaultType {
    Vec,
    IndexMap,
    Option,
    Result,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct DefaultCollection {
    name: &'static str,
    container: Option<DefaultType>,
    mutable_reference: bool,
}

pub(crate) fn written_default_collection(
    ty: Node<'_>,
    identities: &HashMap<Range, DefaultType>,
) -> Option<DefaultCollection> {
    let ty = crate::syntax::unparen_type(ty);
    let (ty, mutable_reference) = if ty.kind() == "reference_type" {
        if !children(ty).iter().any(|child| child.kind() == "mutable_specifier") {
            return None;
        }
        (crate::syntax::unparen_type(ty.child_by_field_name("type")?), true)
    } else {
        (ty, false)
    };
    let head = |ty: Node<'_>| identities.get(&range(ty.child_by_field_name("type")?)).copied();
    if ty.kind() != "generic_type" {
        return None;
    }
    let (inner, container) = match head(ty)? {
        outer @ (DefaultType::Option | DefaultType::Result) => {
            let args = ty.child_by_field_name("type_arguments")?;
            (crate::syntax::unparen_type(named_children(args).into_iter().find(|arg| !arg.is_extra())?), Some(outer))
        }
        _ => (ty, None),
    };
    if inner.kind() != "generic_type" {
        return None;
    }
    let name = match head(inner)? {
        DefaultType::Vec => {
            let args = inner.child_by_field_name("type_arguments")?;
            if named_children(args).into_iter().filter(|arg| !arg.is_extra()).count() != 1 {
                return None;
            }
            "Vec"
        }
        DefaultType::IndexMap => "IndexMap",
        _ => return None,
    };
    Some(DefaultCollection { name, container, mutable_reference })
}

#[derive(Default)]
pub(crate) struct Evidence<'a> {
    pub operations: Operations,
    pub fields: HashMap<Range, TypeInfo>,
    pub direct_fields: HashMap<Range, bool>,
    pub non_bindings: HashSet<Range>,
    pub types: &'a [TypeFact],
    pub type_identities: Option<&'a TypeIdentities>,
    pub cast_aliases: HashMap<Range, NumericAlias>,
    pub default_types: Option<&'a HashMap<Range, DefaultType>>,
    pub default_collections: HashMap<Range, DefaultCollection>,
    pub definitions: Option<&'a HashMap<&'a str, Range>>,
    pub uses: &'a [(usize, usize, &'a str)],
    pub precise_captures: Option<bool>,
}

struct Capture {
    declaration: Range,
    name: String,
    projection: bool,
    info: Option<TypeInfo>,
}

struct Reader<'a, 'e> {
    src: Src<'a>,
    evidence: &'e Evidence<'e>,
    types: HashMap<Range, &'e TypeInfo>,
    readings: Vec<ReadingFact>,
    borrows: Vec<BorrowFact>,
    inferred: Vec<TypeFact>,
}

pub(crate) fn collect(
    src: Src<'_>,
    root: Node<'_>,
    evidence: &Evidence<'_>,
) -> (Vec<ReadingFact>, Vec<BorrowFact>, Vec<TypeFact>) {
    let mut types = HashMap::new();
    for fact in evidence.types {
        let key = (fact.site.start as usize, fact.site.end as usize);
        types
            .entry(key)
            .and_modify(|info: &mut Option<&TypeInfo>| {
                if info.is_some_and(|ty| ty.shape != fact.info.shape) {
                    *info = None;
                }
            })
            .or_insert(Some(&fact.info));
    }
    let mut reader = Reader {
        src,
        evidence,
        types: types
            .into_iter()
            .filter_map(|(key, info)| Some((key, info?)))
            .collect(),
        readings: Vec::new(),
        borrows: Vec::new(),
        inferred: Vec::new(),
    };
    let mut nodes = vec![root];
    while let Some(node) = nodes.pop() {
        if node.is_extra() || node.is_error() || node.is_missing() || node.kind() == "token_tree" {
            continue;
        }
        reader.visit(node);
        nodes.extend(named_children(node).into_iter().rev());
    }
    (reader.readings, reader.borrows, reader.inferred)
}

fn branch(label: &str, action: impl Into<String>, output: impl Into<String>) -> ReadingBranch {
    ReadingBranch {
        label: label.into(),
        action: action.into(),
        output: output.into(),
        pin: None,
    }
}

fn detail(label: &str, text: impl Into<String>) -> ReadingContent {
    ReadingContent::Detail {
        label: label.into(),
        text: text.into(),
        definition: None,
        borrow: false,
    }
}

fn reference(shape: TypeShape) -> TypeShape {
    TypeShape::Indirect {
        pointer: false,
        target: Box::new(shape),
    }
}

fn option(shape: TypeShape) -> TypeInfo {
    TypeInfo::new(
        TypeShape::Named {
            name: "Option".into(),
            arguments: vec![shape],
        },
        TypeOrigin::Compiler,
        None,
    )
}

#[derive(Clone, Copy)]
enum CastNumber {
    Integer {
        name: &'static str,
        bits: u32,
        signed: bool,
    },
    Float {
        name: &'static str,
        bits: u32,
        precision: u32,
    },
}

impl CastNumber {
    fn from_shape(shape: &TypeShape) -> Option<Self> {
        let TypeShape::Scalar { name } = shape else {
            return None;
        };
        let (name, bits, signed) = match name.as_str() {
            "Int8" => ("i8", 8, true),
            "Int16" => ("i16", 16, true),
            "Int32" => ("i32", 32, true),
            "Int64" => ("i64", 64, true),
            "Int128" => ("i128", 128, true),
            "UInt8" => ("u8", 8, false),
            "UInt16" => ("u16", 16, false),
            "UInt32" => ("u32", 32, false),
            "UInt64" => ("u64", 64, false),
            "UInt128" => ("u128", 128, false),
            "Float32" => {
                return Some(Self::Float {
                    name: "f32",
                    bits: 32,
                    precision: 24,
                });
            }
            "Float64" => {
                return Some(Self::Float {
                    name: "f64",
                    bits: 64,
                    precision: 53,
                });
            }
            _ => return None,
        };
        Some(Self::Integer { name, bits, signed })
    }

    fn name(self) -> &'static str {
        match self {
            Self::Integer { name, .. } | Self::Float { name, .. } => name,
        }
    }
}

fn fixed_integer_range(bits: u32, signed: bool) -> String {
    let maximum = u128::MAX >> (128 - bits + u32::from(signed));
    let minimum = if signed {
        format!("-{}", maximum + 1)
    } else {
        "0".into()
    };
    format!("{minimum} through {maximum}")
}

fn numeric_cast_text(from: CastNumber, to: CastNumber) -> String {
    use CastNumber::{Float, Integer};
    let target = to.name();
    match (from, to) {
        (Float { .. }, Integer { bits, signed, .. }) => {
            let range = fixed_integer_range(bits, signed);
            format!(
                "Drop the fractional part by rounding toward zero. Values outside {range}, including infinities, stop at the nearest limit. NaN becomes 0."
            )
        }
        (Integer { bits: source, .. }, Integer { bits, .. }) if source > bits => format!(
            "Keep the lowest {bits} bits and interpret them as {target}. Values outside the destination range can change."
        ),
        (
            Integer {
                bits: source,
                signed,
                ..
            },
            Integer {
                bits,
                signed: target_signed,
                ..
            },
        ) if source == bits => {
            if signed == target_signed {
                "Keep the same value and integer type.".into()
            } else {
                format!(
                    "Keep the same {bits} bits and interpret them as {target}. Changing the sign interpretation can change the numeric value."
                )
            }
        }
        (
            Integer { signed, .. },
            Integer {
                bits,
                signed: target_signed,
                ..
            },
        ) => {
            if signed && !target_signed {
                format!(
                    "Extend the sign bit to {bits} bits, then interpret the result as {target}. A negative input becomes a large positive number."
                )
            } else if signed {
                format!(
                    "Extend the sign bit to {bits} bits. The original numeric value is preserved."
                )
            } else {
                format!(
                    "Fill the new high bits with zeroes to reach {bits} bits. The original numeric value is preserved."
                )
            }
        }
        (Integer { bits, .. }, Float { precision, .. }) if bits <= precision => format!(
            "Convert to floating point. Every {} value fits exactly in {target}.",
            from.name()
        ),
        (Integer { name, .. }, Float { .. }) => {
            let mut text = format!(
                "Use the nearest {target} value. A tie rounds to the result with an even last bit. Large integers can lose precision."
            );
            if name == "u128" && target == "f32" {
                text.push_str(" A result too large after rounding becomes positive infinity.");
            }
            text
        }
        (Float { bits: source, .. }, Float { bits, .. }) if source < bits => format!(
            "Keep every finite value exactly in {target}. Infinity remains infinity and NaN remains NaN."
        ),
        (Float { bits: source, .. }, Float { bits, .. }) if source > bits => format!(
            "Use the nearest {target} value. A tie rounds to the result with an even last bit. Overflow after rounding becomes infinity with the same sign."
        ),
        (Float { .. }, Float { .. }) => "Keep the same floating-point type.".into(),
    }
}

fn cast_name_matches(src: Src<'_>, node: Node<'_>, name: &str) -> bool {
    let written = src.text(node);
    written.strip_prefix("r#").unwrap_or(written) == name
}

fn imports_cast_type(src: Src<'_>, import: Node<'_>, name: &str) -> bool {
    let named =
        |node: Node| cast_name_matches(src, node.child_by_field_name("name").unwrap_or(node), name);
    match import.kind() {
        "use_wildcard" => true,
        "use_as_clause" => import
            .child_by_field_name("alias")
            .is_some_and(|alias| cast_name_matches(src, alias, name)),
        "scoped_use_list" => import.child_by_field_name("list").is_some_and(|list| {
            imports_cast_type(src, list, name)
                || import.child_by_field_name("path").is_some_and(named)
                    && named_children(list)
                        .iter()
                        .any(|item| item.kind() == "self")
        }),
        "use_list" => named_children(import)
            .into_iter()
            .any(|item| imports_cast_type(src, item, name)),
        _ => named(import),
    }
}

pub(crate) fn may_shadow_cast_type(src: Src<'_>, ty: Node<'_>, analyzed: bool) -> bool {
    let name = src.text(ty);
    std::iter::successors(ty.parent(), Node::parent).any(|scope| {
        let named = |node: Node| {
            node.child_by_field_name("name")
                .is_some_and(|n| cast_name_matches(src, n, name))
        };
        if scope
            .child_by_field_name("type_parameters")
            .is_some_and(|parameters| {
                named_children(parameters)
                    .into_iter()
                    .any(|parameter| parameter.kind() == "type_parameter" && named(parameter))
            })
        {
            return true;
        }
        matches!(scope.kind(), "block" | "source_file" | "declaration_list")
            && named_children(scope)
                .into_iter()
                .any(|item| match item.kind() {
                    "type_item" | "struct_item" | "enum_item" | "union_item" | "trait_item"
                    | "mod_item" | "associated_type" | "foreign_type_item" => named(item),
                    "extern_crate_declaration" => item
                        .child_by_field_name("alias")
                        .map_or_else(|| named(item), |alias| cast_name_matches(src, alias, name)),
                    "use_declaration" => item
                        .child_by_field_name("argument")
                        .is_some_and(|import| imports_cast_type(src, import, name)),
                    "macro_invocation" => !analyzed,
                    "expression_statement" if !analyzed => item
                        .named_child(0)
                        .is_some_and(|expr| expr.kind() == "macro_invocation"),
                    _ => false,
                })
    })
}

fn reference_access(ty: &str) -> Access {
    let after = ty.trim_start_matches('&').trim_start();
    let after = if after.starts_with('\'') {
        after
            .split_once(char::is_whitespace)
            .map_or("", |(_, rest)| rest.trim_start())
    } else {
        after
    };
    if after.starts_with("mut ") {
        Access::BorrowMut
    } else {
        Access::Borrow
    }
}

/// The logical receiver and arguments of a method, in either call syntax.
fn receiver(call: Node<'_>) -> Option<(Node<'_>, Option<usize>)> {
    method_call(call)
        .map(|(recv, _)| (recv, None))
        .or_else(|| Some((argument(call, 0)?, Some(0))))
}

fn argument(call: Node<'_>, index: usize) -> Option<Node<'_>> {
    named_children(call.child_by_field_name("arguments")?)
        .into_iter()
        .filter(|node| !node.is_extra() && node.kind() != "attribute_item")
        .nth(index)
        .map(unparen)
}

fn direct_argument_call(mut value: Node<'_>) -> Option<Node<'_>> {
    while let Some(parent) = value.parent().filter(|parent| parent.kind() == "parenthesized_expression") {
        value = parent;
    }
    let arguments = value.parent().filter(|parent| parent.kind() == "arguments")?;
    let call = arguments.parent().filter(|parent| parent.kind() == "call_expression")?;
    (call.child_by_field_name("arguments") == Some(arguments)).then_some(call)
}

#[derive(Clone, Copy)]
enum IteratorCallback {
    FindMap,
    FilterMap,
    TryFold,
}

impl IteratorCallback {
    fn operation(operation: &Resolved) -> Option<Self> {
        if operation.family != Family::Iter {
            return None;
        }
        match operation.name.as_str() {
            "find_map" => Some(Self::FindMap),
            "filter_map" => Some(Self::FilterMap),
            "try_fold" => Some(Self::TryFold),
            _ => None,
        }
    }

    fn is_callback(self, call: Node, closure: Node) -> bool {
        let Some((_, receiver_argument)) = receiver(call) else { return false };
        let index = usize::from(receiver_argument.is_some()) + usize::from(matches!(self, Self::TryFold));
        argument(call, index) == Some(closure)
    }

    fn timing(self) -> &'static str {
        match self {
            Self::FindMap => "While find_map searches",
            Self::FilterMap => "When the iterator requests an item",
            Self::TryFold => "While try_fold processes items",
        }
    }

    fn propagation(self, operand: Option<Family>, option_output: bool) -> Option<(&'static str, &'static str)> {
        match (self, operand) {
            (Self::FindMap, Some(Family::Option)) => Some((
                "Return None from this callback. find_map tries the next item.", "None",
            )),
            (Self::FilterMap, Some(Family::Option)) => Some((
                "Return None from this callback. filter_map skips this item and looks for another value.", "None",
            )),
            (Self::TryFold, Some(Family::Option)) if option_output => Some((
                "Return None from this callback. try_fold stops and returns None.", "None",
            )),
            (Self::TryFold, Some(Family::Option | Family::Result)) => Some((
                "Finish this callback early. try_fold stops and returns the callback's early result.", "Early callback result",
            )),
            _ => None,
        }
    }
}

fn single_type_argument(node: Node<'_>) -> Option<Node<'_>> {
    let args = node.child_by_field_name("type_arguments")?;
    let mut written = named_children(args).into_iter().filter(|argument| !argument.is_extra());
    let first = written.next()?;
    written.next().is_none().then_some(first)
}

fn boundary(node: Node<'_>) -> Option<Node<'_>> {
    std::iter::successors(node.parent(), Node::parent).find(|node| {
        matches!(
            node.kind(),
            "function_item" | "closure_expression" | "async_block" | "try_block"
        )
    })
}

fn callback_scope(node: Node) -> bool {
    std::iter::successors(node.parent(), Node::parent)
        .find(|node| matches!(node.kind(), "function_item" | "closure_expression"))
        .is_some_and(|node| node.kind() == "closure_expression")
}

struct Lookup<'t> {
    receiver: Node<'t>,
    key: Node<'t>,
    input: Option<BorrowInput>,
    value: TypeShape,
    mutable: bool,
}

impl Reader<'_, '_> {
    fn push(&mut self, anchor: Span, span: Span, resolved: bool, content: ReadingContent) {
        let basis = if resolved {
            ValueBasis::TypeAnalysis
        } else {
            ValueBasis::Syntax
        };
        self.readings.push(ReadingFact {
            anchor,
            span,
            basis,
            content,
            expression_target: None,
        });
    }

    fn at(&mut self, node: Node, resolved: bool, content: ReadingContent) {
        let span = self.src.span(node);
        self.push(span.clone(), span, resolved, content);
    }

    fn fact(&mut self, node: Node, rule: BorrowRule) {
        let site = ByteRange {
            file: self.src.file.into(),
            start: node.start_byte() as u32,
            end: node.end_byte() as u32,
        };
        self.borrows.push(BorrowFact { site, rule });
    }

    fn inferred(&mut self, node: Node, info: &TypeInfo) {
        if self.types.contains_key(&range(node)) {
            return;
        }
        let site = ByteRange {
            file: self.src.file.into(),
            start: node.start_byte() as u32,
            end: node.end_byte() as u32,
        };
        self.inferred.push(TypeFact {
            site,
            target: ValueFactTarget::Expression,
            info: info.clone(),
        });
    }

    fn op(&self, node: Node) -> Option<&Resolved> {
        self.evidence.operations.get(&range(node))?.as_ref()
    }

    fn definition(&self, node: Node) -> Option<Range> {
        let index = self
            .evidence
            .uses
            .partition_point(|(start, _, _)| *start < node.start_byte());
        let &(start, end, symbol) = self.evidence.uses.get(index)?;
        if (start, end) != range(node) {
            return None;
        }
        self.evidence.definitions?.get(symbol).copied()
    }

    fn known_type(&self, node: Node) -> Option<TypeInfo> {
        let node = unparen(node);
        if let Some(info) = self
            .types
            .get(&range(node))
            .copied()
            .or_else(|| self.evidence.fields.get(&range(node)))
        {
            return Some(info.clone());
        }
        if node.kind() == "field_expression"
            && let Some((info, _)) = self.tuple_member(node)
        {
            return Some(info);
        }
        if node.kind() == "reference_expression"
            && !children(node).iter().any(|token| token.kind() == "raw")
            && let Some(inner) = node
                .child_by_field_name("value")
                .and_then(|value| self.known_type(value))
        {
            let modifier = if children(node)
                .iter()
                .any(|token| token.kind() == "mutable_specifier")
            {
                "mut "
            } else {
                ""
            };
            let native = inner.native.map(|ty| format!("&{modifier}{ty}"));
            return Some(TypeInfo::new(reference(inner.shape), inner.origin, native));
        }
        if node.kind() == "call_expression"
            && self
                .op(node)
                .is_some_and(|op| op.family == Family::Option && op.name == "Some")
            && let Some(payload) = argument(node, 0).and_then(|value| self.known_type(value))
        {
            return Some(option(payload.shape));
        }
        expression_type(Language::Rust, self.src, node)
            .map(|shape| TypeInfo::new(shape, TypeOrigin::Syntax, None))
    }

    fn lookup<'t>(&self, call: Node<'t>) -> Option<Lookup<'t>> {
        let op = self.op(call)?;
        if !matches!(op.family, Family::Map | Family::Slice)
            || !matches!(op.name.as_str(), "get" | "get_mut")
        {
            return None;
        }
        let (receiver, position) = receiver(call)?;
        let key = argument(call, usize::from(position.is_some()))?;
        let ty = self.known_type(receiver).map(|info| info.shape);
        let by_reference = matches!(&ty, Some(TypeShape::Indirect { pointer: false, .. }));
        let base = match ty.as_ref() {
            Some(TypeShape::Indirect {
                pointer: false,
                target,
            }) => Some(target.as_ref()),
            other => other,
        };
        let items = match (op.family, base) {
            (
                Family::Map,
                Some(TypeShape::Collection {
                    family: CollectionKind::Dictionary,
                    arguments,
                    ..
                }),
            ) => Some(arguments.get(1)),
            // A range lookup on a slice returns a slice. Only a known usize indexes an item.
            (
                Family::Slice,
                Some(TypeShape::Collection {
                    family: CollectionKind::Slice | CollectionKind::Array | CollectionKind::List,
                    arguments,
                    ..
                }),
            ) => {
                let single = self
                    .known_type(key)
                    .is_some_and(|info| info.shape == TypeShape::scalar("UIntSize"))
                    || key.kind() == "integer_literal";
                Some(if single { arguments.first() } else { None })
            }
            _ => None,
        };
        // The declaration proves what get does, but an arbitrary Deref on the receiver
        // can redirect the reference to storage unrelated to that apparent receiver.
        let direct = receiver.kind() != "field_expression"
            || self.evidence.direct_fields.contains_key(&range(receiver));
        let input = (items.is_some() && direct).then_some(BorrowInput {
            argument: position,
            value: by_reference || position.is_some(),
        });
        let value = items.flatten().cloned().unwrap_or(TypeShape::Unknown);
        Some(Lookup {
            receiver,
            key,
            input,
            value,
            mutable: op.name == "get_mut",
        })
    }

    fn lookup_label(&self, lookup: &Lookup) -> String {
        format!(
            "Look up {} in {}",
            self.short(lookup.key),
            self.short(lookup.receiver)
        )
    }

    fn short(&self, node: Node) -> String {
        truncate(&self.src.norm(node), 80)
    }

    fn written(&self, node: Node, limit: usize) -> String {
        truncate(self.src.text(node).trim(), limit)
    }

    fn paths(
        &mut self,
        node: Node,
        span: Span,
        resolved: bool,
        input: String,
        branches: Vec<ReadingBranch>,
        result: Option<TypeInfo>,
    ) {
        let content = self.path_content(node, input, branches, result);
        self.push(self.src.span(node), span, resolved, content);
    }

    fn path_content(
        &self,
        node: Node,
        input: String,
        branches: Vec<ReadingBranch>,
        result: Option<TypeInfo>,
    ) -> ReadingContent {
        let timing = boundary(node).and_then(|scope| match scope.kind() {
            "async_block" => Some("When the future runs".into()),
            "closure_expression" => {
                if children(scope).iter().any(|token| token.kind() == "async") {
                    Some("When the future runs".into())
                } else {
                    let call = direct_argument_call(scope)?;
                    let operation = self.op(call)?;
                    if let Some(callback) = IteratorCallback::operation(operation) {
                        callback.is_callback(call, scope).then(|| callback.timing().into())
                    } else if operation.family == Family::Option && operation.name == "take_if" {
                        let (_, receiver_argument) = receiver(call)?;
                        (argument(call, usize::from(receiver_argument.is_some())) == Some(scope))
                            .then(|| "When the option contains a value".into())
                    } else if operation.family == Family::Bool && operation.name == "then" {
                        let (_, receiver_argument) = receiver(call)?;
                        (argument(call, usize::from(receiver_argument.is_some())) == Some(scope))
                            .then(|| "When the bool is true".into())
                    } else {
                        match operation.family {
                            Family::ArrayFromFn if argument(call, 0) == Some(scope) =>
                                Some("While building the array".into()),
                            Family::IterFromFn if argument(call, 0) == Some(scope) =>
                                Some("On each next() call".into()),
                            Family::Iter | Family::ParIter =>
                                Some("When the iterator requests an item".into()),
                            _ => None,
                        }
                    }
                }
            }
            _ => None,
        });
        ReadingContent::Paths {
            input,
            deferred: callback_scope(node) || timing.is_some(),
            branches,
            result,
            timing,
        }
    }

    fn visit(&mut self, node: Node<'_>) {
        match node.kind() {
            "call_expression" => self.call(node),
            "try_expression" => self.propagation(node),
            "type_cast_expression" => self.numeric_cast(node),
            "array_expression" => self.array_repeat(node),
            "range_expression" => self.range_expression(node),
            "assignment_expression" => {
                if let Some((equal, end)) = inclusive_range_to(node) {
                    let operator = self.src.index.span(self.src.file, node.start_byte(), equal.end_byte());
                    self.describe_range(node, operator, None, Some(end), true);
                }
            }
            "binary_expression" => self.short_circuit(node),
            "let_chain" => self.let_chain(node),
            "while_expression" => self.while_let(node),
            "closure_expression" => self.closure(node),
            "block" => self.block_result(node),
            "base_field_initializer" => self.struct_update(node),
            "macro_invocation" => self.macro_call(node),
            "await_expression" => self.at(node, false, detail("Await a result", "Continue here when the future is ready. If it is pending, this task can suspend.")),
            "unsafe_block" => self.at(node, false, detail("Operations with extra safety conditions", "Check the contract of each unsafe operation used here. Rust still checks the types.")),
            "foreign_mod_item" => self.at(node, false, detail("Foreign interface", "These declarations describe how Rust calls code across an ABI boundary.")),
            "async_block" => {
                let mut text = "The body makes progress when the future is polled.".to_owned();
                let captures = self.captures(node);
                if !captures.is_empty() {
                    let by_value = children(node).iter().any(|token| token.kind() == "move");
                    text.push_str(&self.capture_text(&captures, by_value));
                }
                if children(node).iter().any(|token| token.kind() == "move") {
                    text.push_str(" Captures are taken by value.");
                }
                self.at(node, !captures.is_empty(), detail("Create a future", text));
            }
            "function_item" => {
                if named_children(node).iter().any(|child| {
                    child.kind() == "function_modifiers" && children(*child).iter().any(|token| token.kind() == "async")
                }) {
                    let header = self.src.header_span(node, node.child_by_field_name("body"));
                    self.push(header.clone(), header, false, detail("Async function", "Calling it creates a future. Its body makes progress when the future is polled."));
                }
            }
            "let_declaration" | "let_condition" | "for_expression" => self.binding(node),
            "parameter" if node.parent().is_some_and(|parent| parent.kind() == "parameters") =>
                self.binding(node),
            "match_expression" => {
                self.match_value(node);
                self.match_patterns(node);
            }
            "if_expression" => self.if_value(node),
            "field_expression" => {
                if let Some(by_reference) = self.evidence.direct_fields.get(&range(node)).copied()
                    .or_else(|| self.tuple_member(node).map(|(_, by_reference)| by_reference))
                {
                    self.fact(node, BorrowRule::Field { by_reference });
                }
            }
            _ => {}
        }
    }

    fn array_repeat(&mut self, node: Node<'_>) {
        let Some((value, separator, length)) = nodex_lang_api::syntax_sites::rust_array_repeat(node)
        else {
            return;
        };
        let operand = |mut node: Node<'_>| {
            while node.kind() == "parenthesized_expression" {
                let Some(inner) = named_children(node).into_iter().find(|child| !child.is_extra())
                else {
                    break;
                };
                node = inner;
            }
            (node.kind(), self.src.text(node).trim() == "_")
        };
        let (value_kind, _) = operand(value);
        let written_length = self.written(length, 80);
        let (_, inferred) = operand(length);
        let size = if inferred {
            "Rust infers the array length from context.".into()
        } else {
            format!("The array has {written_length} elements. Rust determines this length at compile time.")
        };
        let behavior = match value_kind {
            "const_block" => {
                "Rust evaluates this const block once for each element. A length of zero skips it. This form also works for element types that cannot be copied."
            }
            "identifier" | "scoped_identifier" | "generic_function" | "macro_invocation" | "self" => {
                "An ordinary value expression is evaluated once, even for an empty array. A named constant or a const block is evaluated once per element and is skipped for an empty array. Repeating an ordinary value more than once requires its type to implement Copy."
            }
            _ => {
                "Rust evaluates the value expression once, even for an empty array. When it completes, its result supplies the elements. Repeating it more than once requires its type to implement Copy."
            }
        };
        self.push(
            self.src.span(node),
            self.src.span(separator),
            false,
            detail("Build an array by repetition", format!("{size} {behavior}")),
        );
    }

    fn numeric_cast(&mut self, node: Node<'_>) {
        if node.has_error() {
            return;
        }
        let Some(target) = node.child_by_field_name("type") else {
            return;
        };
        let Some((to, target_resolved)) = self.written_cast_number(target) else {
            return;
        };
        let Some(value) = node.child_by_field_name("value").map(unparen) else {
            return;
        };
        let Some(operator) = children(node)
            .into_iter()
            .find(|token| token.kind() == "as" && !token.is_missing())
        else {
            return;
        };
        let Some((from, resolved)) = self.cast_source_number(value, 0) else {
            return;
        };
        let label = format!(
            "Convert {} to {}",
            self.cast_type_label(value, from, 0),
            self.cast_type_label(crate::syntax::unparen_type(target), to, 0)
        );
        self.push(
            self.src.span(node),
            self.src.span(operator),
            resolved || target_resolved,
            detail(&label, numeric_cast_text(from, to)),
        );
    }

    fn cast_type_label(&self, node: Node<'_>, number: CastNumber, depth: usize) -> String {
        if let Some(alias) = self
            .evidence
            .cast_aliases
            .get(&range(node))
            .filter(|alias| alias.primitive == number.name())
        {
            return format!("{} ({})", alias.spelling, number.name());
        }
        if depth < 12 {
            let origin = match node.kind() {
                "identifier" => self.cast_binding_origin(node),
                "type_cast_expression" => node.child_by_field_name("type"),
                _ => None,
            };
            if let Some(origin) = origin {
                return self.cast_type_label(unparen(origin), number, depth + 1);
            }
        }
        number.name().into()
    }

    fn cast_binding_origin<'t>(&self, value: Node<'t>) -> Option<Node<'t>> {
        let (declaration, leaf) = self.binding_declaration(value)?;
        let pattern = declaration.child_by_field_name("pattern")?;
        let origin = declaration
            .child_by_field_name("type")
            .or_else(|| declaration.child_by_field_name("value"))?;
        project_pattern(self.src, pattern, origin, leaf)
    }

    fn binding_declaration<'t>(&self, value: Node<'t>) -> Option<(Node<'t>, Node<'t>)> {
        let (start, end) = self.definition(value)?;
        let root = std::iter::successors(Some(value), Node::parent).last()?;
        let leaf = root.descendant_for_byte_range(start, end)?;
        let declaration = std::iter::successors(leaf.parent(), Node::parent)
            .find(|parent| matches!(parent.kind(), "let_declaration" | "parameter"))?;
        Some((declaration, leaf))
    }

    fn cast_source_number(&self, value: Node<'_>, depth: usize) -> Option<(CastNumber, bool)> {
        if depth > 12 {
            return None;
        }
        let value = unparen(value);
        if let Some(alias) = self.evidence.cast_aliases.get(&range(value)) {
            return CastNumber::from_shape(&syntax_shape(Language::Rust, alias.primitive))
                .map(|from| (from, true));
        }
        match value.kind() {
            "identifier" => {
                // The short type in a SCIP local signature can also name an enum.
                let info = self.known_type(value)?;
                let origin = crate::syntax::unparen_type(self.cast_binding_origin(value)?);
                let written = self
                    .written_cast_number(origin)
                    .or_else(|| self.cast_source_number(origin, depth + 1))?
                    .0;
                let agrees = CastNumber::from_shape(&info.shape)
                    .is_some_and(|inferred| inferred.name() == written.name());
                (agrees
                    || matches!(&info.shape, TypeShape::Named { name, .. }
                    if self.evidence.cast_aliases.get(&range(origin)).is_some_and(|alias|
                        alias.spelling.rsplit("::").next() == Some(name.as_str())
                    )))
                .then_some((written, true))
            }
            "type_cast_expression" => {
                let ty = value.child_by_field_name("type")?;
                self.written_cast_number(ty)
            }
            "unary_expression" if children(value).iter().any(|token| token.kind() == "-") => {
                let literal = named_children(value)
                    .into_iter()
                    .find(|child| matches!(child.kind(), "integer_literal" | "float_literal"))?;
                self.cast_source_number(literal, depth + 1)
            }
            "field_expression" => {
                let info = self.evidence.fields.get(&range(value))?;
                CastNumber::from_shape(&info.shape).map(|from| (from, true))
            }
            "binary_expression" => {
                let info = self.types.get(&range(value))?;
                if info.origin != TypeOrigin::Compiler {
                    return None;
                }
                CastNumber::from_shape(&info.shape).map(|from| (from, true))
            }
            "call_expression" | "integer_literal" | "float_literal" => {
                let info = self.known_type(value)?;
                CastNumber::from_shape(&info.shape)
                    .map(|from| (from, info.origin == TypeOrigin::Compiler))
            }
            _ => None,
        }
    }

    fn written_cast_number(&self, ty: Node<'_>) -> Option<(CastNumber, bool)> {
        let ty = crate::syntax::unparen_type(ty);
        if let Some(alias) = self.evidence.cast_aliases.get(&range(ty)) {
            return CastNumber::from_shape(&syntax_shape(Language::Rust, alias.primitive))
                .map(|number| (number, true));
        }
        if let Some((_, identity)) = self
            .evidence
            .type_identities
            .and_then(|types| types.get(&ty.start_byte()))
            .filter(|(end, _)| *end == ty.end_byte())
        {
            return CastNumber::from_shape(&syntax_shape(Language::Rust, identity))
                .map(|number| (number, true));
        }
        if ty.kind() != "primitive_type"
            || may_shadow_cast_type(self.src, ty, self.evidence.type_identities.is_some())
        {
            return None;
        }
        CastNumber::from_shape(&syntax_shape(Language::Rust, self.src.text(ty)))
            .map(|number| (number, false))
    }

    fn block_result(&mut self, block: Node<'_>) {
        let Some(owner) = block.parent() else { return };
        let (label, explanation) = match owner.kind() {
            "function_item" | "for_expression" | "while_expression" | "loop_expression"
            | "if_expression" | "else_clause" | "gen_block" | "try_block" => return,
            "let_declaration" if owner.child_by_field_name("alternative") == Some(block) => return,
            kind @ ("async_block" | "closure_expression")
                if kind == "async_block" || children(owner).iter().any(|token| token.kind() == "async") => (
                "Future result",
                "If this final expression completes, its value becomes the result of the future.",
            ),
            "closure_expression" => (
                "Closure result",
                "If this final expression completes, its value becomes the result of calling the closure.",
            ),
            "match_arm" => (
                "Value of this match arm",
                "If this final expression completes, its value becomes the value of the selected match arm.",
            ),
            "const_block" => (
                "Compile-time block value",
                "If this block is evaluated, Rust computes its value from the final expression during compilation.",
            ),
            _ => (
                "Value from the final expression",
                "If this final expression completes, its value becomes the block's value.",
            ),
        };
        let Some(expression) = block_statements(block).next_back().and_then(statement_tail) else { return };
        let operand = unparen(expression);
        if expression.has_error() || matches!(operand.kind(), "return_expression" | "break_expression" | "continue_expression"
            | "yield_expression" | "loop_expression" | "while_expression" | "for_expression" | "macro_invocation")
        {
            return;
        }
        let mut text = explanation.to_owned();
        if operand.kind() == "compound_assignment_expr"
            || operand.kind() == "assignment_expression" && inclusive_range_to(operand).is_none()
        {
            text.push_str(" This assignment produces () (unit).");
        }
        self.at(expression, false, detail(label, text));
    }

    fn match_value(&mut self, node: Node<'_>) {
        if node.has_error() {
            return;
        }
        let (Some(value), Some(body)) = (node.child_by_field_name("value"), node.child_by_field_name("body")) else {
            return;
        };
        let Some(keyword) = children(node).into_iter().find(|token| token.kind() == "match") else { return };
        let branches = named_children(body).into_iter().filter(|arm| arm.kind() == "match_arm")
            .enumerate().filter_map(|(i, arm)| {
                let pattern = arm.child_by_field_name("pattern")?;
                let expression = arm.child_by_field_name("value")?;
                let label = format!("Arm {} · {}", i + 1, self.written(pattern, 80));
                Some(self.value_branch(&label, expression, "match"))
            }).collect::<Vec<_>>();
        if !branches.is_empty() {
            self.expression_paths(node, keyword, Concept::Match,
                format!("Compare {} with the arm patterns in order", self.written(value, 80)), branches);
        }
    }

    fn if_value(&mut self, node: Node<'_>) {
        if node.has_error() {
            return;
        }
        let (Some(condition), Some(consequence)) = (node.child_by_field_name("condition"), node.child_by_field_name("consequence")) else {
            return;
        };
        let Some(keyword) = children(node).into_iter().find(|token| token.kind() == "if") else { return };
        let when_true = self.value_branch("If the condition passes", consequence, "if expression");
        let when_false = node.child_by_field_name("alternative")
            .and_then(|clause| named_children(clause).into_iter().find(|child| !child.is_extra()))
            .map_or_else(
                || branch("If the condition fails", "With no else branch, this route produces () (unit) if it completes.", "()"),
                |alternative| self.value_branch("If the condition fails", alternative, "if expression"),
            );
        self.expression_paths(node, keyword, Concept::Choose,
            format!("Evaluate the condition {} to choose a result", self.written(condition, 80)), vec![when_true, when_false]);
    }

    fn value_branch(&self, label: &str, expression: Node, choice: &str) -> ReadingBranch {
        let block = unparen(expression);
        if block.kind() == "block" && named_children(block).iter().any(|child| child.kind() == "label") {
            return branch(label, format!("If this labeled block completes, its value becomes the {choice} result."), self.written(expression, 240));
        }
        let (last, tail) = if block.kind() == "block" {
            let last = block_statements(block).next_back();
            (last, last.and_then(statement_tail))
        } else {
            (Some(expression), Some(expression))
        };
        let last_expression = last.and_then(|last| {
            if last.kind() == "expression_statement" {
                named_children(last).into_iter().find(|child| !child.is_extra())
            } else {
                Some(last)
            }
        });
        match (last_expression, tail) {
            (Some(exit), _) if matches!(unparen(exit).kind(), "return_expression" | "break_expression" | "continue_expression") => {
                branch(label, format!("If execution reaches this expression, it leaves the path before the {choice} produces a value."), self.written(exit, 240))
            }
            (_, Some(value)) => branch(label,
                format!("If this expression completes normally, its value becomes the {choice} result."),
                self.written(value, 240)),
            (_, None) => branch(label,
                "If this branch is selected and its block reaches the end, it supplies () (unit).", "()"),
        }
    }

    fn expression_paths(&mut self, node: Node, keyword: Node, target: Concept, input: String, branches: Vec<ReadingBranch>) {
        let content = self.path_content(node, input, branches, None);
        self.readings.push(ReadingFact {
            anchor: self.src.span(node),
            span: self.src.span(keyword),
            basis: ValueBasis::Syntax,
            content,
            expression_target: Some(target),
        });
    }

    fn range_expression(&mut self, node: Node<'_>) {
        if node.parent().is_some_and(|parent| {
            parent.child_by_field_name("left") == Some(node) && inclusive_range_to(parent).is_some()
        }) {
            return;
        }
        let tokens = children(node);
        let Some(operator) = tokens.iter().find(|token| matches!(token.kind(), ".." | "..=")) else {
            return;
        };
        let bounds = tokens.iter().copied().filter(|token| token.is_named() && !token.is_extra());
        let start = bounds.clone().find(|bound| bound.end_byte() <= operator.start_byte());
        // These control expressions consume the range as their value, even if the grammar gives
        // the range the outer node.
        if start.is_some_and(|start| matches!(start.kind(), "return_expression" | "break_expression" | "continue_expression" | "yield_expression")) {
            return;
        }
        let mut bounds = bounds;
        let end = bounds.find(|bound| bound.start_byte() >= operator.end_byte());
        self.describe_range(node, self.src.span(*operator), start, end, operator.kind() == "..=");
    }

    fn describe_range(&mut self, node: Node, operator: Span, start: Option<Node>, end: Option<Node>, inclusive: bool) {
        let (label, text) = match (start, end, inclusive) {
            (Some(start), Some(end), true) => (
                "Range includes the end",
                format!("Start at {} and include {} as the end bound.", self.short(start), self.short(end)),
            ),
            (Some(start), Some(end), false) => (
                "Range stops before the end",
                format!("Start at {} and stop before {}.", self.short(start), self.short(end)),
            ),
            (None, Some(end), true) => (
                "Range includes the end",
                format!("Include {} as the end bound. When slicing an array, slice, or string, an omitted start means its beginning.", self.short(end)),
            ),
            (None, Some(end), false) => (
                "Range has no start bound",
                format!("Stop before {}. When slicing an array, slice, or string, an omitted start means its beginning.", self.short(end)),
            ),
            (Some(start), None, _) => (
                "Range has no end bound",
                format!("Start at {} with no specified end. When slicing an array, slice, or string, an omitted end means through its end.", self.short(start)),
            ),
            (None, None, _) => (
                "Range has no bounds",
                "Neither endpoint is specified. When slicing an array, slice, or string, this selects the whole value.".into(),
            ),
        };
        self.push(self.src.span(node), operator, false, detail(label, text));
    }

    fn struct_update(&mut self, node: Node<'_>) {
        let Some(constructor) = node.parent().and_then(|fields| fields.parent())
            .filter(|parent| parent.kind() == "struct_expression") else { return };
        if node.has_error() {
            return;
        }
        let tokens = children(node);
        let Some(operator) = tokens.iter().find(|token| token.kind() == ".." && !token.is_missing()) else { return };
        let Some(base) = tokens.iter().find(|token| token.is_named() && !token.is_extra()) else { return };
        self.push(
            self.src.span(constructor),
            self.src.span(*operator),
            false,
            detail("Fill omitted fields from the base struct", format!(
                "Evaluate any written field values in source order, then evaluate {} as the base. Rust takes each omitted field from that base. It copies the field if its type implements Copy, or moves it into the new struct otherwise.",
                self.written(*base, 120),
            )),
        );
    }

    fn short_circuit(&mut self, node: Node<'_>) {
        let Some(operator) = node.child_by_field_name("operator") else { return };
        let (skip, check) = match operator.kind() {
            "&&" => ("false", "true"),
            "||" => ("true", "false"),
            _ => return,
        };
        let (Some(left), Some(right)) = (node.child_by_field_name("left"), node.child_by_field_name("right")) else {
            return;
        };
        self.paths(
            node,
            self.src.span(operator),
            false,
            format!("Check {} first", self.short(left)),
            vec![
                branch(&format!("Left side is {skip}"), "Skip the right side", skip),
                branch(&format!("Left side is {check}"), format!("Evaluate {}", self.short(right)), "Result of the right side"),
            ],
            None,
        );
    }

    fn while_let(&mut self, node: Node<'_>) {
        let (Some(condition), Some(body)) = (node.child_by_field_name("condition").map(unparen), node.child_by_field_name("body")) else { return };
        let Some(keyword) = children(node).into_iter().find(|token| token.kind() == "while" && !token.is_missing()) else { return };
        if condition.has_error() {
            return;
        }
        let (input, mut entered, mut ended, resolved) = match condition.kind() {
            "let_condition" => {
                let (Some(pattern), Some(value)) = (condition.child_by_field_name("pattern"), condition.child_by_field_name("value")) else { return };
                let known_by_syntax = self.always_matching_pattern(pattern, false);
                let known_by_resolution = !known_by_syntax && self.evidence.definitions.is_some()
                    && self.always_matching_pattern(pattern, true);
                let irrefutable = known_by_syntax || known_by_resolution;
                let source = unparen(value);
                let awaits = source.kind() == "await_expression";
                let received = awaits && named_children(source).into_iter()
                    .find(|operand| !operand.is_extra()).and_then(|operand| self.op(unparen(operand)))
                    .is_some_and(|op| op.family == Family::Effect && op.name == "recv_optional");
                let value = self.written(value, 120);
                let mut input = if irrefutable {
                    format!("On each attempt, evaluate {value}. This pattern accepts every value it produces.")
                } else {
                    format!("On each attempt, evaluate {value} and check whether it matches {}.", self.written(pattern, 120))
                };
                if awaits {
                    input.push_str(" The task can suspend while waiting. The pattern is checked when the future produces a value.");
                }
                if received {
                    input.push_str(" This receive yields Some(message) for a message, and None when the channel is closed and empty.");
                }
                (
                    input,
                    branch(if irrefutable { "Value is ready" } else { "Pattern matches" },
                        "Run the loop body with any names bound by this pattern.", "Run this iteration"),
                    (!irrefutable).then(|| branch("Pattern does not match", "End the loop without running this iteration.", "Leave the loop")),
                    known_by_resolution || received,
                )
            }
            "let_chain" => (
                "On each attempt, start with the first condition. Check the rest in order while all earlier checks pass.".into(),
                branch("All conditions pass", "Run the loop body with the names bound by the matching patterns.", "Run this iteration"),
                Some(branch("A condition fails", "Skip the remaining checks and end the loop before running the body again.", "Leave the loop")),
                false,
            ),
            _ => return,
        };
        entered.pin = Some("body".into());
        if let Some(branch) = &mut ended {
            branch.pin = Some("out".into());
        }
        let content = ReadingContent::Paths {
            input,
            deferred: false,
            branches: std::iter::once(entered).chain(ended).collect(),
            result: None,
            timing: Some("Before each loop iteration".into()),
        };
        self.push(self.src.header_span(node, Some(body)), self.src.span(keyword), resolved, content);
    }

    fn always_matching_pattern(&self, pattern: Node<'_>, resolve: bool) -> bool {
        match pattern.kind() {
            "_" => true,
            "identifier" => resolve && self.evidence.definitions.is_some_and(|definitions| definitions.values().any(|definition| *definition == range(pattern))),
            "ref_pattern" | "mut_pattern" => patterns(pattern).last().is_some_and(|binding| {
                binding.kind() == "identifier" || self.always_matching_pattern(*binding, resolve)
            }),
            "reference_pattern" => patterns(pattern).last().is_some_and(|pattern| self.always_matching_pattern(*pattern, resolve)),
            "tuple_pattern" => patterns(pattern).iter().all(|pattern| self.always_matching_pattern(*pattern, resolve)),
            "remaining_field_pattern" => true,
            _ => false,
        }
    }

    fn let_chain(&mut self, node: Node<'_>) {
        let Some(operator) = children(node).into_iter().find(|token| token.kind() == "&&") else {
            return;
        };
        let is_while = while_let_header(node).is_some();
        let mut steps: Vec<_> = named_children(node)
            .into_iter()
            .filter(|check| !check.is_extra())
            .enumerate()
            .map(|(index, check)| {
                let action = if let (Some(pattern), Some(value)) = (
                    check.child_by_field_name("pattern"),
                    check.child_by_field_name("value"),
                ) && check.kind() == "let_condition" {
                    format!("Check whether {} matches {}. Any names bound by this pattern are available in later checks and in the body.", self.short(value), self.short(pattern))
                } else {
                    format!("Check whether {} is true.", self.short(check))
                };
                let timing = if index == 0 {
                    if is_while { "First on each attempt" } else { "First" }
                } else { "If all earlier checks pass" };
                self.step(check, timing, action)
            })
            .collect();
        let failure = if is_while { "Skip the remaining checks and leave the loop." }
            else { "Skip the remaining checks and the body guarded by the condition." };
        steps.push(self.step(operator, "If a check fails", failure));
        self.push(
            self.src.span(node),
            self.src.span(operator),
            false,
            ReadingContent::Steps { label: "Check each condition in order".into(), steps },
        );
    }

    fn tuple_member(&self, node: Node<'_>) -> Option<(TypeInfo, bool)> {
        let position: usize = self
            .src
            .text(node.child_by_field_name("field")?)
            .parse()
            .ok()?;
        let base = self.known_type(node.child_by_field_name("value")?)?;
        let (tuple, by_reference) = match &base.shape {
            TypeShape::Indirect {
                pointer: false,
                target,
            } => (target.as_ref(), true),
            other => (other, false),
        };
        let TypeShape::Tuple { items } = tuple else {
            return None;
        };
        Some((
            TypeInfo::new(items.get(position)?.clone(), base.origin, None),
            by_reference,
        ))
    }

    fn propagation(&mut self, node: Node<'_>) {
        let Some(operand) = named_children(node).into_iter().next() else {
            return;
        };
        let Some(token) = children(node).into_iter().find(|token| token.kind() == "?") else {
            return;
        };
        let scope = boundary(node);
        let kind = self.op(node).map(|op| op.family);
        let iterator_exit = scope.filter(|scope| scope.kind() == "closure_expression")
            .and_then(|closure| {
                let call = direct_argument_call(closure)?;
                let callback = IteratorCallback::operation(self.op(call)?)?;
                if !callback.is_callback(call, closure) || children(closure).iter().any(|token| token.kind() == "async") {
                    return None;
                }
                let option_output = matches!(callback, IteratorCallback::TryFold)
                    && kind == Some(Family::Option)
                    && self.closure_returns_option(closure);
                callback.propagation(kind, option_output)
            });
        let exit = iterator_exit.map_or_else(
            || match scope {
                Some(boundary) if boundary.kind() == "closure_expression"
                    && crate::structure::closure_binding(self.src, boundary).is_some() =>
                {
                    "Return from this closure".into()
                }
                Some(boundary) if boundary.kind() == "closure_expression" => {
                    "Return from this callback".into()
                }
                Some(boundary) if boundary.kind() == "async_block" => "Finish this async block".into(),
                Some(boundary) if boundary.kind() == "try_block" => "Finish this try block".into(),
                Some(boundary) => format!(
                    "Return from {}",
                    boundary
                        .child_by_field_name("name")
                        .map_or("this function", |name| self.src.text(name))
                ),
                None => "Leave this scope".into(),
            },
            |(action, _)| action.into(),
        );
        let mut branches = match kind {
            Some(Family::Option) => vec![
                branch("Found", "Unwrap Some", "Continue with the value"),
                branch("Missing", exit, "None"),
            ],
            Some(Family::Result) => vec![
                branch("Success", "Unwrap Ok", "Continue with the value"),
                branch("Error", exit, self.error_output(node, operand)),
            ],
            _ => vec![
                branch("Continue", "Take the value", "Continue"),
                branch("Propagate", exit, "Propagate early"),
            ],
        };
        if let Some((_, output)) = iterator_exit {
            branches[0].output = "Continue this callback with the value".into();
            branches[1].output = output.into();
        } else {
            if let Some(resume) = question_mark_condition(node).or_else(|| question_mark_consumer(node)) {
                branches[0].output = resume.into();
            } else {
                branches[0].pin = Some("out".into());
            }
            branches[1].pin = Some("error".into());
        }
        self.paths(
            unparen(operand),
            self.src.span(token),
            kind.is_some(),
            self.short(operand),
            branches,
            None,
        );
    }

    fn closure_returns_option(&self, closure: Node<'_>) -> bool {
        if let Some(ty) = closure.child_by_field_name("return_type") {
            let ty = crate::syntax::unparen_type(ty);
            return ty.kind() == "generic_type"
                && ty.child_by_field_name("type").is_some_and(|name| {
                    self.evidence.type_identities
                        .and_then(|types| types.get(&name.start_byte()))
                        .is_some_and(|(end, path)| *end == name.end_byte() && path == "core::option::Option")
                });
        }
        closure_result(closure).and_then(|result| self.op(unparen(result)))
            .is_some_and(|op| op.family == Family::Option && op.name == "Some")
    }

    fn error_output(&self, node: Node, operand: Node) -> String {
        let error_type = |info: TypeInfo| match info.shape {
            TypeShape::Named { name, arguments } if name == "Result" => arguments.into_iter().nth(1),
            _ => None,
        };
        let incoming = self.known_type(unparen(operand)).and_then(error_type);
        let outgoing = boundary(node).filter(|scope| scope.kind() == "function_item")
            .and_then(|scope| scope.child_by_field_name("return_type"))
            .and_then(|ty| self.known_type(ty)).and_then(error_type);
        match (incoming, outgoing) {
            (Some(from), Some(to)) if from != to && from != TypeShape::Unknown && to != TypeShape::Unknown =>
                "Convert the error to the return error type with From, then return Err".into(),
            _ => "Propagate the error".into(),
        }
    }

    fn steps(&mut self, node: Node, label: &str, steps: Vec<ReadingStep>) {
        self.at(node, true, ReadingContent::Steps { label: label.into(), steps });
    }

    fn step(&self, at: Node, timing: &str, action: impl Into<String>) -> ReadingStep {
        ReadingStep { span: self.src.span(at), timing: timing.into(), action: action.into() }
    }

    fn call(&mut self, node: Node<'_>) {
        self.collect_type_arguments(node);
        let Some(op) = self.op(node) else { return };
        let family = op.family;
        let name = op.name.clone();
        match family {
            Family::Bool => self.bool_call(node, &name),
            Family::Option | Family::Result => self.container_call(node, family, &name),
            Family::ArrayFromFn | Family::IterFromFn => self.from_fn(node, family),
            Family::MemReplace => self.mem_replace(node, &name),
            Family::NumericFrom { source, target } | Family::NumericTryFrom { source, target } => {
                self.numeric_conversion(node, source, target, matches!(family, Family::NumericTryFrom { .. }));
            }
            Family::Map | Family::Slice if matches!(name.as_str(), "get" | "get_mut") => {
                self.get(node)
            }
            Family::Map if matches!(name.as_str(), "or_default" | "or_insert" | "or_insert_with") => self.map_entry(node, &name),
            Family::Slice if name == "split_at_mut" => {
                if let Some((base, position)) = receiver(node)
                    && let Some(value) = self.slice_receiver(base)
                    && let Some(mid) = argument(node, usize::from(position.is_some()))
                {
                    self.fact(node, BorrowRule::Split {
                        input: BorrowInput { argument: position, value },
                        at: self.short(mid),
                    });
                }
                if let Some((label, description)) = operation_detail(family, &name) {
                    self.at(node, true, detail(label, description));
                }
            }
            Family::Effect if name == "timeout_at" => {
                if let Some(recv) = argument(node, 1)
                    && self.op(unparen(recv)).is_some_and(|op| op.family == Family::Effect && op.name == "recv_optional")
                    && node.parent().is_some_and(|parent| parent.kind() == "await_expression")
                {
                    let wait = node.parent().unwrap();
                    self.paths(wait, self.src.span(node), true, "Wait for a message or the deadline".into(), vec![
                        branch("Message received", "Take the message from the channel", "Ok(Some(message))"),
                        branch("Channel closed", "Continue without a message", "Ok(None)"),
                        branch("Deadline elapsed", "Stop waiting for the message", "Err(Elapsed)"),
                    ], None);
                }
            }
            Family::Iter | Family::ParIter if matches!(name.as_str(), "collect" | "sum" | "count" | "for_each") => {
                self.sequence(node, &name);
            }
            _ => {
                if let Some((label, description)) = operation_detail(family, &name) {
                    self.at(node, true, detail(label, description));
                }
            }
        }
    }

    fn proven_default_collection(&self, value: Node<'_>, depth: usize) -> Option<DefaultCollection> {
        if depth >= 12 {
            return None;
        }
        let value = unparen(value);
        if let Some(proof) = self.evidence.default_collections.get(&range(value)) {
            return Some(*proof);
        }
        match value.kind() {
            "identifier" => {
                let (declaration, binding) = self.binding_declaration(value)?;
                let mut pattern = declaration.child_by_field_name("pattern")?;
                if pattern.kind() == "mut_pattern" {
                    pattern = named_children(pattern).into_iter().find(|child| child.kind() == "identifier")?;
                }
                if pattern != binding {
                    return None;
                }
                written_default_collection(declaration.child_by_field_name("type")?, self.evidence.default_types?)
            }
            "reference_expression"
                if children(value).iter().any(|child| child.kind() == "mutable_specifier") =>
            {
                let mut proof = self.proven_default_collection(value.child_by_field_name("value")?, depth + 1)?;
                if proof.mutable_reference { return None; }
                proof.mutable_reference = true;
                Some(proof)
            }
            "call_expression" if self.op(value).is_some_and(|operation| operation.family == Family::MemReplace) =>
                self.mem_value_collection(value, depth + 1),
            _ => None,
        }
    }

    fn mem_value_collection(&self, call: Node<'_>, depth: usize) -> Option<DefaultCollection> {
        let callee = call.child_by_field_name("function")?;
        if callee.kind() == "generic_function" {
            // A written type argument can request a DerefMut coercion of the destination.
            let args = callee.child_by_field_name("type_arguments")?;
            let ty = named_children(args).into_iter().find(|ty| !ty.is_extra())?;
            let proof = written_default_collection(ty, self.evidence.default_types?)?;
            return (!proof.mutable_reference).then_some(proof);
        }
        let mut proof = self.proven_default_collection(argument(call, 0)?, depth)?;
        if !proof.mutable_reference {
            return None;
        }
        proof.mutable_reference = false;
        Some(proof)
    }

    fn mem_replace(&mut self, node: Node<'_>, name: &str) {
        let Some(callee) = node.child_by_field_name("function") else { return };
        let callee = if callee.kind() == "generic_function" {
            let Some(callee) = callee.child_by_field_name("function") else { return };
            callee
        } else {
            callee
        };
        let method = callee.child_by_field_name("name").unwrap_or(callee);
        let Some(destination) = argument(node, 0) else { return };
        let place = if destination.kind() == "reference_expression" {
            destination.child_by_field_name("value")
        } else {
            None
        };
        let target = place.map_or_else(
            || format!("the place referred to by {}", self.written(destination, 80)),
            |place| self.written(place, 80),
        );
        let (label, action) = if name == "take" {
            let default = self.mem_value_collection(node, 0)
                .filter(|proof| proof.container.is_none())
                .map_or_else(|| "the type's default value".into(), |proof| format!("an empty {}", proof.name));
            (
                "Take the previous value and install the default",
                format!("Compute {default} with Default::default(), then put it into {target}."),
            )
        } else {
            let Some(replacement) = argument(node, 1) else { return };
            (
                "Replace and return the previous value",
                format!("Evaluate the destination first, then {}. Once both are ready, put the new value into {target}.", self.written(replacement, 80)),
            )
        };
        let steps = vec![
            self.step(destination, "Destination if the call returns", action),
            self.step(node, "Returned value", format!("Return the previous value from {target} without dropping it in this call.")),
        ];
        self.push(
            self.src.span(node),
            self.src.span(method),
            true,
            ReadingContent::Steps { label: label.into(), steps },
        );
    }

    fn from_fn(&mut self, node: Node<'_>, family: Family) {
        let Some(callback) = argument(node, 0) else { return };
        let (label, steps) = if family == Family::ArrayFromFn {
            (
                "Fill an array by index",
                vec![
                    self.step(callback, "During this call", "Call the function in index order starting at 0. Each successful call supplies the element at that index before construction proceeds. An empty array requires zero calls."),
                    self.step(node, "If construction completes", "Return the completed array."),
                ],
            )
        } else {
            (
                "Create an iterator from a function",
                vec![
                    self.step(node, "During this call", "Create the iterator and store the function for later."),
                    self.step(callback, "On each next() call", "Run the function. Some(value) supplies one item. None means this request produced no item. The next request calls the function again, even after None, and its return value can change."),
                ],
            )
        };
        self.steps(node, label, steps);
    }

    fn numeric_conversion(&mut self, node: Node<'_>, source: &str, target: &str, checked: bool) {
        let Some(callee) = node.child_by_field_name("function") else { return };
        let method = callee.child_by_field_name("name").unwrap_or(callee);
        let span = self.src.span(method);
        if !checked {
            self.push(
                self.src.span(node),
                span,
                true,
                detail(
                    &format!("Convert {source} to {target}"),
                    format!("Preserve the numeric value and return it directly as {target}."),
                ),
            );
            return;
        }

        let range = match CastNumber::from_shape(&syntax_shape(Language::Rust, target)) {
            Some(CastNumber::Integer { bits, signed, .. }) => {
                format!("the range {}", fixed_integer_range(bits, signed))
            }
            _ => format!("the range of {target} on the target platform"),
        };
        let label = format!("Try converting {source} to {target}");
        if matches!(source, "isize" | "usize") || matches!(target, "isize" | "usize") {
            let mut text = String::new();
            if matches!(source, "isize" | "usize") {
                text = format!("The range of {source} depends on the target platform. ");
            }
            text.push_str(&format!(
                "If the number fits in {range}, return Ok({target} value). If it falls outside that range, return Err(TryFromIntError)."
            ));
            self.push(self.src.span(node), span, true, detail(&label, text));
            return;
        }
        self.paths(
            node,
            span,
            true,
            label,
            vec![
                branch(
                    "Fits",
                    format!("If the number fits in {range}, preserve its value."),
                    format!("Ok({target} value)"),
                ),
                branch(
                    "Out of range",
                    "If the number is outside the destination range, return a conversion error.",
                    "Err(TryFromIntError)",
                ),
            ],
            None,
        );
    }

    fn bool_call(&mut self, node: Node<'_>, name: &str) {
        let Some((condition, receiver_argument)) = receiver(node) else { return };
        let Some(value) = argument(node, usize::from(receiver_argument.is_some())) else { return };
        let condition = self.written(condition, 80);
        let value = self.written(value, 120);
        let (input, branches) = match name {
            "then" => (
                format!("Evaluate {condition} first, then evaluate the callback argument {value}. The bool controls whether the callback runs."),
                vec![
                    branch("True", "Call the callback. If it completes, wrap its result in Some.", "Some(callback result)"),
                    branch("False", "Skip calling the callback", "None"),
                ],
            ),
            "then_some" => (
                format!("Evaluate {condition} first, then evaluate {value} even when the bool is false. If both evaluations complete, choose the result."),
                vec![
                    branch("True", "Wrap the evaluated argument in Some", "Some(value)"),
                    branch("False", "Discard the evaluated argument", "None"),
                ],
            ),
            _ => return,
        };
        self.paths(node, self.src.span(node), true, input, branches, None);
    }

    fn collect_type_arguments(&mut self, node: Node<'_>) {
        let Some(generic) = node.child_by_field_name("function").filter(|callee| callee.kind() == "generic_function") else {
            return;
        };
        if !method_call(node).is_some_and(|(_, name)| matches!(self.src.text(name), "collect" | "r#collect")) {
            return;
        }
        let Some(args) = generic.child_by_field_name("type_arguments").filter(|args| !args.has_error()) else {
            return;
        };
        let Some(separator) = children(generic).into_iter().find(|part| part.kind() == "::") else {
            return;
        };
        let written = self.src.norm(args);
        let Some(written) = written.strip_prefix('<').and_then(|text| text.strip_suffix('>')) else {
            return;
        };
        if written.trim().is_empty() {
            return;
        }
        let mut text = format!("This call supplies {} in the generic argument list ::<...>.", truncate(written.trim(), 80));
        let mut pending = named_children(args);
        while let Some(part) = pending.pop() {
            if self.src.text(part) == "_"
                && (part.kind() == "type_identifier" || part.parent().is_some_and(|parent| {
                    parent.kind() == "array_type" && parent.child_by_field_name("length") == Some(part)
                }))
            {
                text.push_str(" The _ asks Rust to infer that part from the surrounding code.");
                break;
            }
            if !matches!(part.kind(), "block" | "token_tree") {
                pending.extend(named_children(part));
            }
        }
        let span = self.src.index.span(self.src.file, separator.start_byte(), args.end_byte());
        self.push(self.src.span(node), span, false, detail("Supply generic arguments to collect", text));
    }

    fn sequence(&mut self, node: Node, consumer: &str) {
        let Some((mut source, _)) = receiver(node) else { return };
        let option_list = consumer == "collect" && self.collects_option_list(node, unparen(source));
        let mut steps = Vec::new();
        while let Some(operation) = self.op(unparen(source)).filter(|op| matches!(op.family, Family::Iter | Family::ParIter)) {
            let action = match operation.name.as_str() {
                "iter" => "Read each item through a shared reference",
                "iter_mut" => "Read each item through a reference that permits changes",
                "into_iter" => "Take the iterator source by value",
                "par_iter" => "Read items by reference, allowing parallel processing",
                "par_iter_mut" => "Read disjoint items by mutable reference, allowing parallel processing",
                "into_par_iter" => "Take the source by value for parallel processing",
                "zip" => "Pair one item with an item from the other input",
                "chain" => "Read the first input, then the second",
                "flatten" => "Yield the items from each inner sequence",
                "filter" => "Keep the item if its test passes",
                "filter_map" => "Run the callback. Yield the value for Some and skip this item for None",
                "map" => "Run the callback with this item. A return in the callback ends that call",
                _ => break,
            };
            steps.push(self.step(source, "When consumed", action));
            let Some((previous, _)) = receiver(unparen(source)) else { break };
            source = previous;
        }
        if steps.is_empty() {
            self.at(node, true, detail("Consume the iterator", "Drive the iterator to produce a result."));
            return;
        }
        steps.reverse();
        let final_step = if option_list {
            self.step(node, "When collecting", "If an item is None, stop asking for items and return None. If the iterator reaches the end, return Some(list) containing the unwrapped values. The list can be empty.")
        } else {
            self.step(node, "Now", format!("Drive the sequence with {consumer}."))
        };
        steps.push(final_step);
        self.steps(node, "Follow an item through the iterator", steps);
    }

    fn collects_option_list(&self, collect: Node, mapped: Node) -> bool {
        let standard_call = |node, family, name| {
            self.op(node).is_some_and(|op| op.family == family && op.name == name)
        };
        if !standard_call(collect, Family::Iter, "collect") || !standard_call(mapped, Family::Iter, "map") {
            return false;
        }
        let Some((_, receiver_argument)) = receiver(mapped) else { return false };
        let Some(closure) = argument(mapped, usize::from(receiver_argument.is_some())) else { return false };
        if closure.kind() != "closure_expression" || children(closure).iter().any(|token| token.kind() == "async") {
            return false;
        }
        if !closure.child_by_field_name("body").map(unparen)
            .is_some_and(|body| standard_call(body, Family::Option, "copied"))
        {
            return false;
        }

        let Some(option) = collect.child_by_field_name("function").and_then(single_type_argument) else { return false };
        let Some(list) = single_type_argument(option) else { return false };
        let exact_name = |application: Node| {
            let name = application.child_by_field_name("type")?;
            let (end, path) = self.evidence.type_identities?.get(&name.start_byte())?;
            (*end == name.end_byte()).then_some(path.as_str())
        };
        exact_name(option) == Some("core::option::Option")
            && matches!(exact_name(list), Some("alloc::vec::Vec" | "std::vec::Vec"))
            && single_type_argument(list).is_some()
    }

    fn receiver_is_reference(&self, receiver: Node) -> bool {
        self.known_type(unparen(receiver)).is_some_and(|info| {
            matches!(info.shape, TypeShape::Indirect { pointer: false, .. })
        })
    }

    fn direct_option_receiver(&self, receiver: Node<'_>) -> Option<bool> {
        let receiver = unparen(receiver);
        if receiver.kind() == "field_expression"
            && !self.evidence.direct_fields.contains_key(&range(receiver))
            && self.tuple_member(receiver).is_none()
        {
            return None;
        }
        let info = self.known_type(receiver)?;
        let (shape, by_reference) = match &info.shape {
            TypeShape::Indirect { pointer: false, target } => (target.as_ref(), true),
            shape => (shape, false),
        };
        if matches!(shape, TypeShape::Named { name, .. } if name == "core::option::Option") {
            return Some(by_reference);
        }
        if !matches!(shape, TypeShape::Named { name, .. } if name == "Option") {
            return None;
        }
        let mut written = self.cast_binding_origin(receiver)?;
        if self.op(written).is_some_and(|operation| {
            operation.family == Family::Option && operation.name == "Some"
        }) {
            return Some(by_reference);
        }
        if written.kind() == "reference_type" {
            written = written.child_by_field_name("type")?;
        }
        let written = crate::syntax::unparen_type(written);
        let name = written.child_by_field_name("type").filter(|_| written.kind() == "generic_type")?;
        self.evidence.type_identities
            .and_then(|identities| identities.get(&name.start_byte()))
            .is_some_and(|(end, path)| *end == name.end_byte() && path == "core::option::Option")
            .then_some(by_reference)
    }

    fn slice_receiver(&self, receiver: Node) -> Option<bool> {
        let receiver = unparen(receiver);
        if receiver.kind() == "field_expression"
            && !self.evidence.direct_fields.contains_key(&range(receiver))
            && self.tuple_member(receiver).is_none()
        {
            return None;
        }
        let info = self.known_type(receiver)?;
        let (shape, by_reference) = match &info.shape {
            TypeShape::Indirect { pointer: false, target } => (target.as_ref(), true),
            shape => (shape, false),
        };
        // A resolved slice method can also be reached through a user-defined DerefMut.
        let proven = match shape {
            TypeShape::Collection { family: CollectionKind::Array | CollectionKind::Slice, .. } => true,
            TypeShape::Collection { family: CollectionKind::List, .. } => info.origin != TypeOrigin::Syntax,
            _ => false,
        };
        proven.then_some(by_reference)
    }

    fn map_entry(&mut self, node: Node, name: &str) {
        let Some((entry, _)) = receiver(node) else { return };
        let entry = unparen(entry);
        if !self.op(entry).is_some_and(|op| op.family == Family::Map && op.name == "entry") {
            return;
        }
        let Some((map, position)) = receiver(entry) else { return };
        let Some(key) = argument(entry, usize::from(position.is_some())) else { return };
        let place = format!("{}[{}]", self.short(map), self.short(key));
        self.fact(entry, BorrowRule::Lookup {
            input: BorrowInput { argument: position, value: self.receiver_is_reference(map) },
            projection: format!("[{}]", self.short(key)),
        });
        let position = receiver(node).and_then(|(_, position)| position);
        self.fact(node, BorrowRule::Payload { input: BorrowInput { argument: position, value: true } });
        let missing = match name {
            "or_insert" => "Insert the supplied value",
            "or_insert_with" => "Call the initializer and insert its result",
            _ => "Create and insert the default value",
        };
        self.paths(node, self.src.span(node), true, format!("Find or create {place}"), vec![
            branch("Key exists", "Keep the existing entry", "Mutable reference to the entry"),
            branch("Key missing", missing, "Mutable reference to the entry"),
        ], self.known_type(node));
        self.at(node, true, ReadingContent::Detail {
            label: "Borrow the entry to change it".into(), text: format!("Both paths give exclusive access to {place}."),
            definition: None, borrow: true,
        });
    }

    fn get(&mut self, node: Node<'_>) {
        let Some(lookup) = self.lookup(node) else {
            return;
        };
        let key = self.short(lookup.key);
        if let Some(input) = lookup.input {
            self.fact(
                node,
                BorrowRule::Lookup {
                    input,
                    projection: format!("[{key}]"),
                },
            );
        }
        let result = option(reference(lookup.value.clone()));
        self.inferred(node, &result);
        let use_ = if lookup.mutable {
            "Borrow the entry to allow changes"
        } else {
            "Borrow the entry for reading"
        };
        let place = format!("{}[{key}]", self.short(lookup.receiver));
        self.at(
            node,
            true,
            ReadingContent::Detail {
                label: "Borrowed entry".into(),
                text: format!("{use_} from {place} when it exists."),
                definition: None,
                borrow: true,
            },
        );
        let combined = std::iter::successors(node.parent(), Node::parent)
            .take_while(|parent| {
                matches!(
                    parent.kind(),
                    "parenthesized_expression"
                        | "field_expression"
                        | "generic_function"
                        | "arguments"
                        | "call_expression"
                )
            })
            .find(|parent| parent.kind() == "call_expression")
            .is_some_and(|parent| {
                self.op(parent)
                    .is_some_and(|op| op.family == Family::Option && op.name == "map")
                    && receiver(parent).is_some_and(|(recv, _)| unparen(recv).id() == node.id())
            });
        if !combined {
            self.paths(
                node,
                self.src.span(node),
                true,
                self.lookup_label(&lookup),
                vec![
                    branch("Found", use_, "Some(reference)"),
                    branch("Missing", "There is no entry", "None"),
                ],
                Some(result),
            );
        }
    }

    fn container_call(&mut self, node: Node<'_>, family: Family, name: &str) {
        if matches!(name, "Some" | "Ok") {
            self.fact(node, BorrowRule::Wrap);
            return;
        }
        let Some((recv, position)) = receiver(node) else {
            return;
        };
        let callback_arg = usize::from(position.is_some());
        let callback =
            argument(node, callback_arg).filter(|node| node.kind() == "closure_expression");
        let eager_insert = family == Family::Option && name == "get_or_insert";
        if family == Family::Option && matches!(name, "get_or_insert" | "get_or_insert_with") {
            let direct = self.direct_option_receiver(recv);
            if let Some(by_reference) = direct {
                self.fact(node, BorrowRule::Lookup {
                    input: BorrowInput { argument: position, value: by_reference },
                    projection: " (present contents)".into(),
                });
                self.at(node, true, ReadingContent::Detail {
                    label: "Borrow the stored value to change it".into(),
                    text: format!("Both paths give a mutable reference to the contents of {}.", self.short(recv)),
                    definition: None, borrow: true,
                });
            }
        }
        if matches!(name, "map" | "and_then") && callback.is_some() {
            self.fact(
                node,
                BorrowRule::Callback {
                    input: BorrowInput {
                        argument: position,
                        value: true,
                    },
                    argument: callback_arg,
                },
            );
        }
        if matches!(name, "unwrap" | "expect") {
            self.fact(
                node,
                BorrowRule::Payload {
                    input: BorrowInput {
                        argument: position,
                        value: true,
                    },
                },
            );
        }
        let mut input = match (family, name) {
            (Family::Option, "ok_or") => {
                let Some(error) = argument(node, callback_arg) else { return };
                format!("If {} produces an Option, evaluate the error argument {} before checking whether the option is Some or None.",
                    self.written(recv, 120), self.written(error, 120))
            }
            (Family::Option, "replace") => format!("Replace the contents of {}", self.short(recv)),
            (Family::Option, "get_or_insert") => format!("Keep or fill {}", self.short(recv)),
            (Family::Option, "take_if") => format!("Take the contents of {} if the test passes", self.short(recv)),
            (Family::Option | Family::Result, "unwrap_or_default") => format!("Read {} or use its default", self.short(recv)),
            _ => self.short(recv),
        };
        let mut span = self.src.span(node);
        if matches!((family, name), (Family::Option, "replace" | "get_or_insert" | "take_if" | "unwrap_or_default") | (Family::Result, "unwrap_or_default")) {
            let member = method_call(node).map(|(_, member)| member).or_else(|| {
                let callee = node.child_by_field_name("function")?;
                let callee = if callee.kind() == "generic_function" { callee.child_by_field_name("function")? } else { callee };
                callee.child_by_field_name("name")
            });
            if let Some(member) = member {
                span = self.src.span(member);
            }
        }
        if eager_insert && let Some(value) = argument(node, callback_arg) {
            self.steps(node, "Evaluate the candidate before checking the option", vec![
                self.step(value, "Before the method runs", format!("Evaluate {} even if the option already contains a value.", self.written(value, 80))),
            ]);
        }
        if family == Family::Option && name == "unwrap_or_else"
            && self.op(unparen(recv)).is_some_and(|op| op.family == Family::Option && op.name == "take")
            && let Some((place, _)) = receiver(unparen(recv))
        {
            self.steps(node, "Take the value or create an alternative", vec![
                self.step(recv, "Now", format!("Take the previous contents of {} and leave it empty (None).", self.short(place))),
                self.step(node, "If it held a value", "Use that value directly."),
                self.step(callback.unwrap_or(node), "If it was empty", "Run the fallback and use its result."),
            ]);
        }
        let mut result = self.known_type(node);
        let mut paths = match family {
            Family::Option => option_paths(name),
            Family::Result => result_paths(name),
            _ => return,
        };
        if name == "unwrap_or_default" {
            let container = if family == Family::Option { DefaultType::Option } else { DefaultType::Result };
            if let Some(proof) = self.proven_default_collection(recv, 0)
                .filter(|proof| !proof.mutable_reference && proof.container == Some(container))
                && let Some(fallback) = paths.get_mut(1)
            {
                fallback.output = format!("Empty {}", proof.name);
            }
        }
        let returned = callback.and_then(closure_result);
        if let Some(ty) = returned.and_then(|expr| self.known_type(expr)) {
            match (family, name) {
                (Family::Option, "map") => result = Some(option(ty.shape)),
                (Family::Option | Family::Result, "and_then")
                    if result.is_none()
                        && matches!(&ty.shape, TypeShape::Named { name, .. } if
                        (family == Family::Option && name == "Option") || (family == Family::Result && name == "Result")) =>
                {
                    result = Some(ty)
                }
                _ => {}
            }
        }
        if matches!(name, "map" | "and_then")
            && callback.is_some()
            && result.as_ref().is_some_and(|ty| {
                matches!(&ty.shape, TypeShape::Named { name, arguments }
                if matches!(name.as_str(), "Option" | "Result")
                && matches!(arguments.first(), Some(TypeShape::Indirect { pointer: false, .. })))
            })
        {
            self.at(node, true, ReadingContent::Detail {
                label: "Callback returns a reference".into(),
                text: "When the callback supplies a reference, its result determines what is borrowed.".into(),
                definition: None, borrow: true,
            });
        }
        if family == Family::Option && name == "map" {
            if let Some(lookup) = self.lookup(unparen(recv)) {
                input = self.lookup_label(&lookup);
                paths[0].label = "Found".into();
                if let Some(returned) = returned.filter(|node| node.kind() == "identifier")
                    && let Some(pattern) = callback.and_then(first_parameter)
                {
                    let parts = self.pattern_parts(pattern, None, None);
                    if let Some(part) = parts.iter().find(|part| {
                        part.projection.starts_with('.')
                            && part.access == Some(Access::Copy)
                            && !part.type_info.as_ref().is_some_and(|info| {
                                matches!(info.shape, TypeShape::Indirect { .. })
                            })
                            && self.definition(returned)
                                == Some((
                                    part.span.start_byte as usize,
                                    part.span.end_byte as usize,
                                ))
                    }) {
                        let item = if part.projection == ".0" {
                            "first tuple item".into()
                        } else {
                            format!("item {}", part.projection)
                        };
                        paths[0].action = format!("Copy the {item} as {}", self.src.text(returned));
                    }
                    paths[0].output = format!("Some({})", self.src.text(returned));
                }
            }
        }
        if paths.is_empty() {
            return;
        }
        if let Some(info) = &result {
            self.inferred(node, info);
        }
        self.paths(node, span, true, input, paths, result);
    }

    fn macro_call(&mut self, node: Node<'_>) {
        let Some(name) = node.child_by_field_name("macro") else {
            return;
        };
        let resolved = self.op(node).filter(|op| op.family == Family::Macro);
        let is_select = resolved.is_some_and(|op| op.name == "select");
        let is_vec = resolved.is_some_and(|op| op.name == "vec");
        if is_select {
            self.select_branches(node);
        }
        let repeated_vec = is_vec && self.vec_repeat(node);
        let resolved = self.op(node).filter(|op| op.family == Family::Macro);
        let (label, text) = match resolved.map(|op| op.name.as_str()) {
            Some("vec") if repeated_vec => ("Build a list", "This macro creates a Vec by repeating one value."),
            Some("vec") => (
                "Build a list",
                "This macro creates a Vec with the specified items.",
            ),
            Some("format") => (
                "Format text",
                "This macro builds a String from a format and its values.",
            ),
            Some("assert") => (
                "Check a condition",
                "If the condition is false, the generated check panics.",
            ),
            Some("matches") => (
                "Test a pattern",
                "Try the written pattern. A | between patterns offers alternatives. The result is true if a pattern matches and any if guard passes. Otherwise the result is false.",
            ),
            Some("select") => (
                "Wait for one eligible branch",
                "Poll the eligible futures in the current task. When a future completes and its pattern matches, stop polling the other futures and run that branch's handler. A handler that finishes normally supplies the result of select!.",
            ),
            _ => (
                "Macro invocation",
                "Rust expands this macro during compilation using the tokens written at the call.",
            ),
        };
        let definition = resolved.and_then(|op| op.definition.clone());
        self.push(
            self.src.span(node),
            self.src.span(name),
            resolved.is_some(),
            ReadingContent::Detail {
                label: label.into(),
                text: text.into(),
                definition,
                borrow: false,
            },
        );
    }

    fn vec_repeat(&mut self, node: Node<'_>) -> bool {
        let Some(tree) = named_children(node).into_iter().find(|child| child.kind() == "token_tree") else { return false };
        if tree.has_error() {
            return false;
        }
        let tokens: Vec<_> = children(tree).into_iter().filter(|token| !token.is_extra()).collect();
        let [_, arguments @ .., _] = tokens.as_slice() else { return false };
        let mut separators = arguments.iter().enumerate().filter(|(_, token)| token.kind() == ";");
        let Some((index, separator)) = separators.next().filter(|_| separators.next().is_none()) else { return false };
        let Some((value, value_span)) = vec_repeat_operand(self.src, &arguments[..index]) else { return false };
        let Some((length, length_span)) = vec_repeat_operand(self.src, &arguments[index + 1..]) else { return false };
        let steps = vec![
            ReadingStep {
                span: value_span,
                timing: "Evaluate the value once".into(),
                action: format!("Evaluate {} once, even when the length is zero.", truncate(value, 120)),
            },
            ReadingStep {
                span: length_span,
                timing: "Read the vector length".into(),
                action: format!("Use {} as the number of elements. This length can be computed at runtime.", truncate(length, 120)),
            },
            self.step(*separator, "Fill the vector", "Repeat the value using its type's Clone implementation. Elements can share data if Clone shares it. A length of zero drops the value and leaves the vector empty."),
        ];
        self.push(self.src.span(node), self.src.span(*separator), true, ReadingContent::Steps {
            label: "Build a vector by repetition".into(), steps,
        });
        true
    }

    fn select_branches(&mut self, node: Node) {
        let Some(tree) = named_children(node).into_iter().find(|child| child.kind() == "token_tree") else { return };
        if tree.has_error() {
            return;
        }
        let tokens: Vec<_> = children(tree).into_iter().filter(|token| !token.is_extra()).collect();
        let loop_body = select_loop_body(node);
        let mut start = if let [_, bias, separator, ..] = tokens.as_slice()
            && self.src.text(*bias) == "biased" && separator.kind() == ";"
        {
            3
        } else {
            1
        };
        let mut steps = Vec::new();
        for (arrow, token) in tokens.iter().enumerate().filter(|(_, token)| self.src.text(**token) == "=>") {
            if arrow < start {
                continue;
            }
            let before = &tokens[start..arrow];
            let body = &tokens[arrow + 1..];
            let exit = loop_body.and_then(|kind| select_loop_exit(body, kind));
            if let Some(equal) = before.iter().position(|token| self.src.text(*token) == "=")
                && let Some(first) = before.get(equal + 1)
                && let Some(last) = before.last()
                && select_branch_pattern(self.src, &before[..equal])
            {
                let wait = self.src.text.get(first.start_byte()..last.end_byte()).unwrap_or("");
                let (at, action) = exit.unwrap_or((*token, "run this branch"));
                steps.push(self.step(at, "When ready and the pattern matches", format!("{wait} → {action}")));
            } else if let [otherwise] = before && self.src.text(*otherwise) == "else" {
                let (at, action) = exit.unwrap_or((*token, "Run the else branch"));
                steps.push(self.step(at, "If all branches are disabled", action));
            } else {
                break;
            }
            start = if body.first().is_some_and(|token| {
                token.kind() == "token_tree" && self.src.text(*token).starts_with('{')
            }) {
                // A bare block handler can omit its comma. The next prefix must still
                // look like a branch before we attribute a handler to it.
                arrow + 2 + usize::from(body.get(1).is_some_and(|token| token.kind() == ","))
            } else {
                select_handler_comma(self.src, body).map_or(tokens.len(), |comma| arrow + 2 + comma)
            };
            if start >= tokens.len() {
                break;
            }
        }
        if !steps.is_empty() {
            self.steps(node, "Wait for one eligible branch", steps);
        }
    }

    fn closure(&mut self, node: Node<'_>) {
        let Some(parameters) = node.child_by_field_name("parameters") else {
            return;
        };
        let tokens = children(node);
        let moves = tokens.iter().any(|token| token.kind() == "move");
        let async_ = tokens.iter().any(|token| token.kind() == "async");
        if !async_ && !moves {
            self.closure_result_borrow(node);
        }
        let mut text = if async_ {
            "Calling this function value creates a future.".to_owned()
        } else {
            "The body runs when this function value is called.".to_owned()
        };
        if moves {
            text.push_str(" Captured values are taken by value. Copyable values, including shared references, are copied.");
        }
        let captures = self.captures(node);
        if !captures.is_empty() {
            text.push_str(&self.capture_text(&captures, moves));
        }
        let function = crate::structure::closure_binding(self.src, node);
        let label = if let Some(name) = function {
            let params = patterns(parameters)
                .into_iter()
                .filter_map(|param| {
                    let pattern = param.child_by_field_name("pattern").unwrap_or(param);
                    (pattern.kind() == "identifier").then(|| self.src.text(pattern))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("Create function {name}({params})")
        } else {
            "Create a function".into()
        };
        self.push(
            self.src.span(node),
            self.src.span(parameters),
            !captures.is_empty(),
            detail(&label, text),
        );
        if !async_ && moves {
            self.closure_result_borrow(node);
        }

        let lookup = self.callback_lookup(node);
        for (i, pattern) in patterns(parameters).into_iter().enumerate() {
            let pattern = pattern.child_by_field_name("pattern").unwrap_or(pattern);
            if pattern.kind() == "identifier" {
                continue;
            }
            let (input, ty, source) = match lookup.as_ref().filter(|_| i == 0) {
                Some(lookup) => {
                    let lookup_span = self.lookup_call(node).map(|call| self.src.span(call));
                    let info =
                        TypeInfo::new(reference(lookup.value.clone()), TypeOrigin::Compiler, None);
                    let input = format!(
                        "Entry {}[{}]",
                        self.short(lookup.receiver),
                        self.short(lookup.key)
                    );
                    (input, Some(info), lookup_span)
                }
                None => (format!("Argument {}", i + 1), None, None),
            };
            self.pattern(self.src.span(node), pattern, None, input, ty, source, None);
        }
    }

    fn closure_result_borrow(&mut self, closure: Node<'_>) {
        let Some(body) = closure.child_by_field_name("body").map(unparen)
            .filter(|body| body.kind() == "reference_expression") else { return };
        let tokens = children(body);
        if tokens.iter().any(|token| token.kind() == "raw") {
            return;
        }
        let Some(ampersand) = tokens.iter().find(|token| token.kind() == "&") else { return };
        let Some(value) = body.child_by_field_name("value").filter(|value| !value.is_missing()) else { return };
        let place = self.short(value);
        let mutable = tokens.iter().find(|token| token.kind() == "mutable_specifier");
        let (label, text) = if mutable.is_some() {
            (
                "Borrow the result for modification",
                format!("This result expression borrows {place} with permission to modify it through the reference."),
            )
        } else {
            (
                "Borrow the result for reading",
                format!("This result expression borrows {place} and creates a shared reference for reading."),
            )
        };
        let end = mutable.unwrap_or(ampersand).end_byte();
        let operator = self.src.index.span(self.src.file, ampersand.start_byte(), end);
        self.push(self.src.span(closure), operator, false, detail(label, text));
    }

    fn captures(&self, node: Node<'_>) -> Vec<Capture> {
        let Some(definitions) = self.evidence.definitions else {
            return Vec::new();
        };
        let first = self
            .evidence
            .uses
            .partition_point(|(start, _, _)| *start < node.start_byte());
        let mut captures = BTreeMap::new();
        for &(start, end, symbol) in self.evidence.uses[first..]
            .iter()
            .take_while(|(start, _, _)| *start < node.end_byte())
        {
            if let Some(&declaration) = definitions.get(symbol)
                && declaration.0 < node.start_byte()
                && let Some(usage) = node.descendant_for_byte_range(start, end)
            {
                let mut place = usage;
                let mut info = self.types.get(&declaration).map(|ty| (*ty).clone());
                while let Some(field) = crate::syntax::field_projection(place).filter(|_| self.evidence.precise_captures != Some(false)) {
                    place = field;
                    // An owned tuple has no implicit dereference or packed fields.
                    info = info.and_then(|base| {
                        let TypeShape::Tuple { items } = base.shape else { return None };
                        let position = self.src.text(field.child_by_field_name("field")?).parse::<usize>().ok()?;
                        Some(TypeInfo::new(items.get(position)?.clone(), base.origin, None))
                    });
                }
                let projection = place != usage;
                let name = self.src.norm(place);
                captures.insert((declaration, name.clone()), Capture { declaration, name, projection, info });
            }
        }
        let whole: HashSet<_> = captures.values().filter(|capture| !capture.projection)
            .map(|capture| capture.declaration).collect();
        captures.into_values().filter(|capture| !capture.projection || !whole.contains(&capture.declaration))
            .take(8).collect()
    }

    fn capture_text(&self, captures: &[Capture], by_value: bool) -> String {
        let (known, uncertain): (Vec<_>, Vec<_>) = captures.iter().partition(|capture| {
            !capture.projection || self.evidence.precise_captures == Some(true) && capture.info.is_some()
        });
        let entries: Vec<_> = known.into_iter().map(|capture| {
            let name = &capture.name;
            if !by_value {
                return name.clone();
            }
            let Some(info) = &capture.info else {
                return format!("{name} by value");
            };
            match (&info.shape, info.native.as_deref()) {
                (TypeShape::Indirect { pointer: false, .. }, Some(native))
                    if crate::flow::is_copy(native, &HashSet::new()) => format!("{name} by copying the shared reference"),
                (TypeShape::Indirect { pointer: false, .. }, _) => format!("{name} by carrying the reference into the capture"),
                (TypeShape::Scalar { .. }, Some(native))
                    if crate::flow::is_copy(native, &HashSet::new()) => format!("{name} by copying the value"),
                (TypeShape::Scalar { name: scalar }, None) if capture.projection
                    && matches!(scalar.as_str(), "Bool" | "Char" | "Int8" | "Int16" | "Int32" | "Int64" | "Int128"
                        | "IntSize" | "UInt8" | "UInt16" | "UInt32" | "UInt64" | "UInt128" | "UIntSize" | "Float32" | "Float64") =>
                    format!("{name} by copying the value"),
                (TypeShape::Indirect { .. }, None) if capture.projection => name.clone(),
                _ => format!("{name} by value"),
            }
        }).collect();
        let mut text = String::new();
        if !entries.is_empty() {
            text.push_str(&format!(" Captures {} from the surrounding scope.", entries.join(", ")));
        }
        if !uncertain.is_empty() {
            let used = uncertain.into_iter().map(|capture| capture.name.as_str()).collect::<Vec<_>>().join(", ");
            text.push_str(&format!(" Uses {used} from the surrounding scope. The exact captured part is unknown."));
        }
        text
    }

    fn lookup_call<'t>(&self, closure: Node<'t>) -> Option<Node<'t>> {
        let args = std::iter::successors(closure.parent(), Node::parent)
            .find(|node| node.kind() != "parenthesized_expression")
            .filter(|node| node.kind() == "arguments")?;
        let call = args
            .parent()
            .filter(|node| node.kind() == "call_expression")?;
        let op = self.op(call)?;
        let (recv, pos) = receiver(call)?;
        (op.family == Family::Option
            && matches!(op.name.as_str(), "map" | "and_then")
            && argument(call, usize::from(pos.is_some())) == Some(closure))
        .then(|| unparen(recv))
    }

    fn callback_lookup<'t>(&self, closure: Node<'t>) -> Option<Lookup<'t>> {
        self.lookup(self.lookup_call(closure)?)
    }

    fn binding(&mut self, node: Node<'_>) {
        let Some(pattern) = node.child_by_field_name("pattern") else {
            return;
        };
        let value = node.child_by_field_name("value");
        if let Some(value) = value
            && node.kind() == "let_declaration"
            && let Some(else_token) = children(node).into_iter().find(|token| token.kind() == "else")
        {
            self.let_else(node, value, pattern, else_token);
        }
        if pattern.kind() == "identifier" {
            return;
        }
        let input = value.map_or_else(|| "Input value".into(), |value| self.short(value));
        let declared = node.child_by_field_name("type");
        let lookup = value.and_then(|value| self.lookup(unparen(value)));
        let ty = declared
            .map(|ty| {
                let text = self.src.text(ty);
                TypeInfo::new(
                    syntax_shape(Language::Rust, text),
                    TypeOrigin::Declared,
                    Some(text.into()),
                )
            })
            .or_else(|| {
                lookup
                    .as_ref()
                    .map(|lookup| option(reference(lookup.value.clone())))
            })
            .or_else(|| value.and_then(|value| self.known_type(value)));
        let route = match node.kind() {
            "let_condition" => Some(if while_let_header(node).is_some() { "body" } else { "true" }.into()),
            "for_expression" => Some("body".into()),
            _ if node.child_by_field_name("alternative").is_some() => Some("out".into()),
            _ => None,
        };
        let anchor = if node.kind() == "parameter" {
            node.parent().and_then(|list| list.parent()).map_or_else(
                || self.src.span(node),
                |function| {
                    self.src
                        .header_span(function, function.child_by_field_name("body"))
                },
            )
        } else if let Some(body) = node
            .child_by_field_name("body")
            .or_else(|| node.child_by_field_name("alternative"))
        {
            self.src.header_span(node, Some(body))
        } else {
            self.src.span(node)
        };
        let source = value
            .filter(|_| lookup.is_some())
            .map(|value| self.src.span(unparen(value)));
        self.pattern(
            anchor,
            pattern,
            value.or(declared),
            input,
            ty,
            source,
            route.map(|pin| (pin, None)),
        );
    }

    fn let_else(&mut self, node: Node, value: Node, pattern: Node, else_token: Node) {
        let mut success = branch("Pattern matches", "Any names bound by the pattern become available after this statement.", "Continue below");
        success.pin = Some("out".into());
        let mut mismatch = branch("Pattern mismatch", "The else block must leave this path or run without finishing. It can use return, break, continue, or panic.", "Run else");
        mismatch.pin = Some("else".into());
        let scope = boundary(node);
        let timing = scope.filter(|scope| scope.kind() == "async_block").map(|_| "When the future runs".into());
        let inline_closure = scope.is_some_and(|scope| {
            scope.kind() == "closure_expression"
                && crate::structure::closure_binding(self.src, scope).is_none()
                && crate::structure::callback_callee(self.src, scope).is_none()
        });
        self.push(
            self.src.span(node),
            self.src.span(else_token),
            false,
            ReadingContent::Paths {
                input: format!("Check whether {} matches {}", self.short(value), self.short(pattern)),
                deferred: inline_closure || timing.is_some(),
                branches: vec![success, mismatch],
                result: None,
                timing,
            },
        );
    }

    fn match_patterns(&mut self, node: Node<'_>) {
        let Some(value) = node.child_by_field_name("value") else {
            return;
        };
        let Some(body) = node.child_by_field_name("body") else {
            return;
        };
        let anchor = self.src.header_span(node, Some(body));
        for (i, arm) in named_children(body)
            .into_iter()
            .filter(|arm| arm.kind() == "match_arm")
            .enumerate()
        {
            let Some(pattern) = arm.child_by_field_name("pattern") else {
                continue;
            };
            let label = format!("Case · {}", self.short(pattern));
            let guard = pattern.child_by_field_name("condition");
            let Some(pattern) = patterns(pattern)
                .into_iter()
                .find(|child| Some(*child) != guard)
            else {
                continue;
            };
            self.pattern(
                anchor.clone(),
                pattern,
                Some(value),
                self.short(value),
                self.known_type(value),
                None,
                Some((format!("case:{i}"), Some(label))),
            );
        }
    }

    fn pattern(
        &mut self,
        anchor: Span,
        pattern: Node<'_>,
        value: Option<Node<'_>>,
        mut input: String,
        input_type: Option<TypeInfo>,
        source: Option<Span>,
        route: Option<(String, Option<String>)>,
    ) {
        if pattern.kind() == "reference_pattern" {
            input = if patterns(pattern)
                .first()
                .is_some_and(|node| node.kind() == "tuple_pattern" && rust_grouped_pattern(*node).is_none())
            {
                format!("Read the referenced tuple · {input}")
            } else {
                format!("Read through the reference · {input}")
            };
        }
        let mut parts = self.pattern_parts(pattern, value, input_type.as_ref());
        if let Some(split) = value.map(unparen).filter(|value| {
            self.op(*value)
                .is_some_and(|op| op.family == Family::Slice && op.name == "split_at_mut")
        }) {
            if let Some((base, pos)) = receiver(split)
                && let Some(mid) = argument(split, usize::from(pos.is_some()))
            {
                input = if self.slice_receiver(base).is_some() {
                    format!("Split the mutable borrow of {}", self.short(base))
                } else {
                    format!("Split the mutable slice reached through {}", self.short(base))
                };
                for part in &mut parts {
                    if let Some(side) = match part.projection.as_str() {
                        ".0" => Some(format!("[..{}]", self.short(mid))),
                        ".1" => Some(format!("[{}..]", self.short(mid))),
                        _ => None,
                    } {
                        part.projection = side;
                        if part.binding.is_some()
                            && !matches!(part.access, Some(Access::Borrow | Access::BorrowMut))
                        {
                            part.access = Some(Access::BorrowMut);
                        }
                    }
                }
            }
        }
        if !parts.is_empty() {
            let resolved = source.is_some()
                || input_type.as_ref().is_some_and(|info| info.origin == TypeOrigin::Compiler)
                || parts.iter().any(|part| {
                    part.condition.is_some() && self.evidence.non_bindings.contains(&(
                        part.span.start_byte as usize, part.span.end_byte as usize,
                    ))
                })
                || parts.iter().any(|part| {
                    part.type_info
                        .as_ref()
                        .is_some_and(|ty| ty.origin == TypeOrigin::Compiler)
                })
                || value.is_some_and(|value| self.op(unparen(value)).is_some());
            let (route, route_label) =
                route.map_or((None, None), |(pin, label)| (Some(pin), label));
            self.push(
                anchor,
                self.src.span(pattern),
                resolved,
                ReadingContent::Pattern {
                    input,
                    input_type,
                    source,
                    route,
                    route_label,
                    parts,
                },
            );
        }
    }

    fn pattern_parts(
        &self,
        pattern: Node<'_>,
        value: Option<Node<'_>>,
        ty: Option<&TypeInfo>,
    ) -> Vec<ReadingPart> {
        let mut parts = Vec::new();
        let mut collect = |leaf: Node<'_>, context: PatternContext<'_>, item: PatternItem| {
            let item = if self.evidence.non_bindings.contains(&range(leaf)) {
                PatternItem::Check(self.short(leaf))
            } else {
                item
            };
            if context.checks_covered && !matches!(item, PatternItem::Bind(_)) {
                return;
            }
            if matches!(item, PatternItem::Bind(_))
                && leaf.kind() == "identifier"
                && context.mode.is_none()
                && !leaf.parent().is_some_and(|parent| {
                    parent.kind() == "mut_pattern"
                        || parent.kind() == "captured_pattern"
                            && parent.named_child(0) == Some(leaf)
                })
                && !self.types.contains_key(&range(leaf))
                && self
                    .src
                    .text(leaf)
                    .chars()
                    .next()
                    .is_some_and(char::is_uppercase)
            {
                return;
            }
            let (binding, condition) = match item {
                PatternItem::Bind(condition) => (Some(self.short(leaf)), condition),
                PatternItem::Check(condition) => (None, Some(condition)),
                PatternItem::Ignore => (None, None),
            };
            let type_info = self
                .types
                .get(&range(leaf))
                .map(|info| (*info).clone())
                .or_else(|| {
                    let projected = project_pattern(self.src, pattern, value?, leaf)?;
                    if matches!(
                        projected.kind(),
                        "primitive_type" | "reference_type" | "tuple_type" | "array_type"
                    ) {
                        let text = self.src.text(projected);
                        return Some(TypeInfo::new(
                            syntax_shape(Language::Rust, text),
                            TypeOrigin::Declared,
                            Some(text.into()),
                        ));
                    }
                    self.known_type(projected)
                });
            let access = binding.as_ref().map(|_| {
                context.mode.or_else(|| {
                    let info = type_info.as_ref()?;
                    if context.borrows
                        && let Some(ty) = info.native.as_deref().filter(|ty| ty.starts_with('&'))
                    {
                        return Some(reference_access(ty));
                    }
                    if info
                        .native
                        .as_deref()
                        .is_some_and(|ty| crate::flow::is_copy(ty, &HashSet::new()))
                        || info.native.is_none()
                            && matches!(&info.shape, TypeShape::Scalar { name } if name != "String")
                    {
                        return Some(Access::Copy);
                    }
                    if matches!(
                        info.shape,
                        TypeShape::Collection {
                            family: CollectionKind::List
                                | CollectionKind::Dictionary
                                | CollectionKind::Set
                                | CollectionKind::Deque
                                | CollectionKind::PriorityQueue,
                            ..
                        }
                    ) || matches!(
                        info.native.as_deref(),
                        Some("std::string::String" | "alloc::string::String")
                    ) {
                        return Some(Access::Move);
                    }
                    None
                })
                .unwrap_or(Access::ByValue)
            });
            let place = if context.projection.is_empty() {
                "Value"
            } else {
                context.projection
            };
            let projection = if context.alternatives.is_empty() {
                place.to_owned()
            } else {
                let selected = context.alternatives.iter().rev().map(String::as_str).collect::<Vec<_>>().join(" within ");
                format!("Selected alternative {selected} · {place}")
            };
            parts.push(ReadingPart {
                projection,
                span: self.src.span(leaf),
                binding,
                condition,
                access,
                type_info,
                borrow_sources: None,
            });
        };
        let inner = std::iter::successors(Some(pattern), |&item| rust_grouped_pattern(item))
            .last().unwrap_or(pattern);
        let borrows = !matches!(inner.kind(), "identifier" | "mut_pattern")
            && ty.is_some_and(|ty| matches!(ty.shape, TypeShape::Indirect { pointer: false, .. }));
        let context = PatternContext {
            projection: "",
            alternatives: &[],
            mode: None,
            borrows,
            checks_covered: false,
        };
        walk_pattern(pattern, context, &mut collect, self.src);
        parts
    }
}

fn while_let_header(mut node: Node<'_>) -> Option<Node<'_>> {
    while let Some(parent) = node.parent().filter(|parent| matches!(parent.kind(), "let_chain" | "parenthesized_expression")) {
        node = parent;
    }
    node.parent().filter(|parent| parent.kind() == "while_expression" && parent.child_by_field_name("condition") == Some(node))
}

fn question_mark_condition(mut node: Node<'_>) -> Option<&'static str> {
    let mut transfer = None;
    while let Some(parent) = node.parent() {
        let field_is = |name| parent.child_by_field_name(name) == Some(node);
        let continuation = match parent.kind() {
            "while_expression" if field_is("condition") => Some("Continue checking the loop condition"),
            "for_expression" if field_is("value") => Some("Continue evaluating what the loop iterates over"),
            "if_expression" if field_is("condition") => Some("Continue checking the if condition"),
            "match_expression" if field_is("value") => Some("Continue evaluating the value to match"),
            "match_pattern" if field_is("condition") => Some("Continue checking the match guard"),
            "let_declaration" if field_is("value") && parent.child_by_field_name("alternative").is_some() =>
                Some("Continue evaluating the initializer before checking the let-else pattern"),
            "return_expression" | "break_expression" | "yield_expression" => {
                transfer = Some("Continue with the value");
                None
            }
            // These bodies have an independent destination for propagation.
            "closure_expression" | "async_block" | "try_block" | "const_block" | "gen_block" | "function_item" => return None,
            _ => None,
        };
        if continuation.is_some() {
            return transfer.or(continuation);
        }
        node = parent;
    }
    None
}

fn question_mark_consumer(mut node: Node<'_>) -> Option<&'static str> {
    while let Some(parent) = node.parent().filter(|parent| parent.kind() == "parenthesized_expression") {
        node = parent;
    }
    let parent = node.parent()?;
    match parent.kind() {
        "arguments" if parent.parent().is_some_and(|call| call.kind() == "call_expression") =>
            Some("Use the value to continue evaluating the call"),
        "call_expression" if parent.child_by_field_name("function") == Some(node) =>
            Some("Use the value to continue evaluating the call"),
        "field_expression" if parent.child_by_field_name("value") == Some(node) => {
            let mut callee = parent;
            if let Some(generic) = parent.parent().filter(|generic| generic.kind() == "generic_function"
                && generic.child_by_field_name("function") == Some(parent)) {
                callee = generic;
            }
            let receiver_of_call = callee.parent().and_then(method_call)
                .is_some_and(|(receiver, _)| receiver == node);
            Some(if receiver_of_call {
                "Use the value to continue evaluating the call"
            } else {
                "Use the value to continue evaluating the field access"
            })
        }
        "unary_expression" | "binary_expression" | "type_cast_expression" | "index_expression"
        | "reference_expression" | "await_expression" | "try_expression" | "tuple_expression"
        | "array_expression" | "field_initializer" | "base_field_initializer"
        | "assignment_expression" | "compound_assignment_expr" | "range_expression" =>
            Some("Use the value to continue evaluating this expression"),
        _ => None,
    }
}

fn vec_repeat_operand<'a>(src: Src<'a>, tokens: &[Node<'_>]) -> Option<(&'a str, Span)> {
    let start = tokens.first()?.start_byte();
    let end = tokens.last()?.end_byte();
    let text = &src.text[start..end];
    // Parsing one let initializer accepts nested punctuation and rejects a list of operands.
    let prefix = "fn operand() { let _ = ";
    let wrapped = format!("{prefix}{text}; }}");
    let parsed = crate::parse(&wrapped).filter(|tree| !tree.root_node().has_error())?;
    let value = parsed.root_node().named_child(0)?.child_by_field_name("body")?
        .named_child(0)?.child_by_field_name("value")?;
    if value.start_byte() != prefix.len() || value.end_byte() != prefix.len() + text.len()
        || unparen(value).utf8_text(wrapped.as_bytes()).ok() == Some("_")
    {
        return None;
    }
    Some((text, src.index.span(src.file, start, end)))
}

fn select_loop_body(mut node: Node<'_>) -> Option<&'static str> {
    while let Some(parent) = node.parent() {
        match parent.kind() {
            "loop_expression" | "while_expression" => {
                return (parent.child_by_field_name("body") == Some(node)).then(|| parent.kind());
            }
            // The iterator expression is outside the scope of this `for`.
            "for_expression" if parent.child_by_field_name("body") == Some(node) => return Some(parent.kind()),
            "block" if named_children(parent).iter().any(|child| child.kind() == "label") => return None,
            "function_item" | "closure_expression" | "async_block" | "gen_block" | "const_block" | "const_item" | "static_item"
            | "macro_invocation" | "token_tree" => return None,
            _ => {}
        }
        node = parent;
    }
    None
}

fn select_loop_exit<'tree>(body: &[Node<'tree>], loop_kind: &str) -> Option<(Node<'tree>, &'static str)> {
    let [handler, next, ..] = body else { return None };
    if next.kind() != "," && !(body.len() == 2 && matches!(next.kind(), "}" | ")" | "]") && !next.is_missing()) {
        return None;
    }
    let action = match handler.kind() {
        "break" => "Leave the enclosing loop. This select! supplies no value on this path.",
        "continue" => match loop_kind {
            "for_expression" => "Skip the rest of this iteration and ask the enclosing loop for the next item. This select! supplies no value on this path.",
            "while_expression" => "Skip the rest of this iteration and check the enclosing loop's condition again. This select! supplies no value on this path.",
            _ => "Skip the rest of this iteration and start the next one. This select! supplies no value on this path.",
        },
        _ => return None,
    };
    Some((*handler, action))
}

fn select_handler_comma(src: Src, body: &[Node<'_>]) -> Option<usize> {
    let first = body.first()?;
    body.iter().enumerate().take_while(|(_, token)| token.kind() != "=>")
        .filter(|(_, token)| token.kind() == ",")
        .find_map(|(index, comma)| {
            let handler = &src.text[first.start_byte()..comma.start_byte()];
            // Top-level token trees also expose commas in closure parameters and generic arguments.
            let wrapped = format!("fn handler() {{ let _ = {handler}; }}");
            crate::parse(&wrapped).filter(|tree| !tree.root_node().has_error()).map(|_| index)
        })
}

fn select_branch_pattern(src: Src, tokens: &[Node<'_>]) -> bool {
    let (Some(first), Some(last)) = (tokens.first(), tokens.last()) else { return false };
    let pattern = &src.text[first.start_byte()..last.end_byte()];
    let wrapped = format!("fn check() {{ match () {{ {pattern} => () }} }}");
    crate::parse(&wrapped).is_some_and(|tree| !tree.root_node().has_error())
}

enum PatternItem {
    Bind(Option<String>),
    Check(String),
    Ignore,
}

#[derive(Clone, Copy)]
struct PatternContext<'a> {
    projection: &'a str,
    alternatives: &'a [String],
    mode: Option<Access>,
    borrows: bool,
    checks_covered: bool,
}

fn pattern_place(parent: &str, part: &str) -> String {
    if parent.is_empty() {
        part.to_owned()
    } else {
        format!("{parent} · {part}")
    }
}

fn rest_pattern(node: Node<'_>) -> bool {
    match node.kind() {
        "remaining_field_pattern" => true,
        "captured_pattern" | "ref_pattern" | "mut_pattern" => patterns(node)
            .last()
            .is_some_and(|child| rest_pattern(*child)),
        _ => false,
    }
}

fn pattern_alternatives<'t>(node: Node<'t>, alternatives: &mut Vec<Node<'t>>) {
    if node.kind() == "or_pattern" {
        for alternative in patterns(node) {
            pattern_alternatives(alternative, alternatives);
        }
    } else {
        alternatives.push(node);
    }
}

fn walk_pattern<'t>(
    node: Node<'t>,
    context: PatternContext<'_>,
    emit: &mut impl FnMut(Node<'t>, PatternContext<'_>, PatternItem),
    src: Src<'_>,
) {
    let projection = context.projection;
    match node.kind() {
        "identifier" | "shorthand_field_identifier" => emit(node, context, PatternItem::Bind(None)),
        "_" | "remaining_field_pattern" => emit(node, context, PatternItem::Ignore),
        "string_literal" | "raw_string_literal" | "char_literal" | "boolean_literal"
        | "integer_literal" | "float_literal" | "negative_literal" | "range_pattern"
        | "scoped_identifier" | "const_block" => {
            if !context.checks_covered {
                emit(node, context, PatternItem::Check(src.text(node).trim().into()));
            }
        }
        "tuple_pattern" | "tuple_struct_pattern" | "slice_pattern" => {
            if let Some(item) = rust_grouped_pattern(node) {
                walk_pattern(item, context, emit, src);
                return;
            }
            let variant = node.child_by_field_name("type");
            let prefix = variant.map_or_else(
                || projection.to_owned(),
                |ty| pattern_place(projection, &src.norm(ty)),
            );
            let items: Vec<_> = patterns(node)
                .into_iter()
                .filter(|child| Some(*child) != variant)
                .collect();
            let mut after_rest = false;
            for (i, &child) in items.iter().enumerate() {
                let item = if rest_pattern(child) {
                    after_rest = true;
                    pattern_place(&prefix, "Remaining items")
                } else if after_rest {
                    let from_end = items.len() - i;
                    let name = if from_end == 1 {
                        "Last item".into()
                    } else {
                        format!("Item {from_end} from the end")
                    };
                    pattern_place(&prefix, &name)
                } else if node.kind() == "slice_pattern" {
                    format!("{prefix}[{i}]")
                } else {
                    format!("{prefix}.{i}")
                };
                walk_pattern(
                    child,
                    PatternContext {
                        projection: &item,
                        ..context
                    },
                    emit,
                    src,
                );
            }
        }
        "struct_pattern" => {
            let owner = node.child_by_field_name("type");
            let prefix = owner.map_or_else(
                || projection.to_owned(),
                |ty| pattern_place(projection, &src.norm(ty)),
            );
            for field in patterns(node)
                .into_iter()
                .filter(|child| Some(*child) != owner)
            {
                let place = if field.kind() == "remaining_field_pattern" {
                    pattern_place(&prefix, "Remaining fields")
                } else {
                    prefix.clone()
                };
                walk_pattern(
                    field,
                    PatternContext {
                        projection: &place,
                        ..context
                    },
                    emit,
                    src,
                );
            }
        }
        "field_pattern" => {
            if let Some(name) = node.child_by_field_name("name") {
                let field = format!("{projection}.{}", src.norm(name));
                let tokens = children(node);
                let mode = if tokens.iter().any(|child| child.kind() == "ref") {
                    Some(
                        if tokens
                            .iter()
                            .any(|child| child.kind() == "mutable_specifier")
                        {
                            Access::BorrowMut
                        } else {
                            Access::Borrow
                        },
                    )
                } else {
                    context.mode
                };
                walk_pattern(
                    node.child_by_field_name("pattern").unwrap_or(name),
                    PatternContext {
                        projection: &field,
                        mode,
                        ..context
                    },
                    emit,
                    src,
                );
            }
        }
        "ref_pattern" => {
            let items = patterns(node);
            let access = if items.iter().any(|child| child.kind() == "mut_pattern") {
                Access::BorrowMut
            } else {
                Access::Borrow
            };
            for child in items {
                walk_pattern(
                    child,
                    PatternContext {
                        mode: Some(access),
                        ..context
                    },
                    emit,
                    src,
                );
            }
        }
        "reference_pattern" | "mut_pattern" => {
            let context = PatternContext {
                borrows: context.borrows && node.kind() != "reference_pattern",
                ..context
            };
            for child in patterns(node) {
                walk_pattern(child, context, emit, src);
            }
        }
        "captured_pattern" => {
            if let [binding, checked] = patterns(node).as_slice() {
                let (binding, checked) = (*binding, *checked);
                if rest_pattern(checked) || checked.kind() == "_" {
                    emit(binding, context, PatternItem::Bind(None));
                    return;
                }
                let inner = std::iter::successors(Some(checked), |&item| rust_grouped_pattern(item))
                    .last()
                    .unwrap_or(checked);
                // An explicit ref on the name to the left of @ applies only to that binding.
                let checked_context = PatternContext { mode: None, ..context };
                if inner.kind() == "or_pattern" {
                    walk_pattern(checked, checked_context, emit, src);
                    emit(binding, context, PatternItem::Bind(None));
                } else {
                    let check = (!context.checks_covered).then(|| src.text(checked).trim().to_owned());
                    emit(binding, context, PatternItem::Bind(check));
                    walk_pattern(
                        checked,
                        PatternContext {
                            checks_covered: true,
                            ..checked_context
                        },
                        emit,
                        src,
                    );
                }
            }
        }
        "or_pattern" => {
            let mut branches = Vec::new();
            pattern_alternatives(node, &mut branches);
            if let [only] = branches.as_slice() {
                walk_pattern(*only, context, emit, src);
                return;
            }
            if !context.checks_covered && !branches.is_empty() {
                let check = branches
                    .iter()
                    .map(|&branch| src.text(branch).trim())
                    .collect::<Vec<_>>()
                    .join(" | ");
                emit(node, context, PatternItem::Check(check));
            }
            for branch in branches {
                let mut alternatives = context.alternatives.to_vec();
                alternatives.push(truncate(src.text(branch).trim(), 80));
                let next = PatternContext {
                    alternatives: &alternatives,
                    checks_covered: true,
                    ..context
                };
                walk_pattern(branch, next, emit, src);
            }
        }
        _ => {}
    }
}

fn first_parameter(closure: Node<'_>) -> Option<Node<'_>> {
    let parameter = patterns(closure.child_by_field_name("parameters")?)
        .into_iter()
        .next()?;
    Some(
        parameter
            .child_by_field_name("pattern")
            .unwrap_or(parameter),
    )
}

fn closure_result(closure: Node<'_>) -> Option<Node<'_>> {
    let body = unparen(closure.child_by_field_name("body")?);
    if body.kind() != "block" {
        return Some(body);
    }
    let last = block_statements(body).next_back()?;
    let expr = if last.kind() == "expression_statement" {
        named_children(last).into_iter().find(|node| !node.is_extra())?
    } else {
        last
    };
    if expr.kind() == "return_expression" {
        return expr.named_child(0);
    }
    statement_tail(last)
}

fn patterns(node: Node<'_>) -> Vec<Node<'_>> {
    children(node)
        .into_iter()
        .filter(|child| {
            !child.is_extra()
                && !child.is_missing()
                && child.kind() != "mutable_specifier"
                && (child.is_named() || child.kind() == "_")
        })
        .collect()
}

fn option_paths(name: &str) -> Vec<ReadingBranch> {
    let (found, output, missing, absent) = match name {
        "is_some_and" | "is_none_or" => (
            "Call the predicate with the value inside Some",
            "The predicate's bool result",
            "Skip calling the predicate",
            if name == "is_some_and" { "false" } else { "true" },
        ),
        "map" => (
            "Run the callback with the value",
            "Some(callback result)",
            "Skip the callback",
            "None",
        ),
        "and_then" => (
            "Run the callback with the value",
            "The callback's Option",
            "Skip the callback",
            "None",
        ),
        "or_else" => (
            "Keep the present value",
            "Some(value)",
            "Run the fallback callback",
            "The callback's Option",
        ),
        "filter" => {
            return vec![
                branch(
                    "Present · passes",
                    "Test the value and keep it",
                    "Some(value)",
                ),
                branch(
                    "Present · rejected",
                    "Test the value and discard it",
                    "None",
                ),
                branch("Missing", "Skip the test", "None"),
            ];
        }
        "ok_or" => (
            "Keep the value inside Some and discard the evaluated error",
            "Ok(value)",
            "Use the already evaluated error",
            "Err(error)",
        ),
        "ok_or_else" => (
            "Keep the present value",
            "Ok(value)",
            "Build the error with the callback",
            "Err(callback result)",
        ),
        "transpose" => {
            return vec![
                branch(
                    "Present · success",
                    "Move the presence inside the result",
                    "Ok(Some(value))",
                ),
                branch("Present · error", "Carry the error through", "Err(error)"),
                branch("Missing", "Succeed with no value", "Ok(None)"),
            ];
        }
        "take" => ("Move the contents out and leave None", "Some(previous value)", "Leave the slot empty", "None"),
        "take_if" => {
            return vec![
                branch("Empty", "Skip the test and leave the option empty", "None"),
                branch(
                    "Present · test returns true",
                    "Run the test with a mutable reference to the value. Move the resulting contents out and leave None.",
                    "Some(taken value)",
                ),
                branch(
                    "Present · test returns false",
                    "Run the test with a mutable reference to the value. Leave the value in the option, including any changes made by the test.",
                    "None",
                ),
            ];
        }
        "replace" => (
            "Store the new value as Some and give back the previous value",
            "Some(previous value)",
            "Store the new value as Some in the empty option",
            "None",
        ),
        "get_or_insert" => (
            "Keep the existing contents and discard the evaluated candidate",
            "Mutable reference to the existing value",
            "Store the evaluated candidate as Some",
            "Mutable reference to the inserted value",
        ),
        "get_or_insert_with" => ("Borrow the existing contents for writing", "Mutable reference", "Call the initializer and store its result", "Mutable reference"),
        "unwrap_or" => (
            "Evaluate the default and keep the present value",
            "value",
            "Evaluate and use the default",
            "default",
        ),
        "unwrap_or_else" => (
            "Keep the present value and skip the fallback",
            "value",
            "Run the fallback callback",
            "callback result",
        ),
        "unwrap_or_default" => (
            "Use the value inside Some",
            "value",
            "Call Default::default() for the value type",
            "default value",
        ),
        "unwrap" | "expect" => (
            "Take the present value",
            "value",
            "Panic",
            "No value is returned",
        ),
        "as_ref" => (
            "Borrow the present value",
            "Some(reference)",
            "Keep the absence",
            "None",
        ),
        "as_deref" => (
            "Borrow through the present value",
            "Some(reference to the target)",
            "Keep the absence",
            "None",
        ),
        "copied" => (
            "Copy the referenced value",
            "Some(copy)",
            "Keep the absence",
            "None",
        ),
        "cloned" => (
            "Clone the referenced value",
            "Some(clone)",
            "Keep the absence",
            "None",
        ),
        _ => return Vec::new(),
    };
    vec![
        branch("Found", found, output),
        branch("Missing", missing, absent),
    ]
}

fn result_paths(name: &str) -> Vec<ReadingBranch> {
    let (success, output, error, failure) = match name {
        "is_ok" => (
            "The Result contains a successful value",
            "true",
            "The Result contains an error",
            "false",
        ),
        "ok" => (
            "Keep the value inside Ok",
            "Some(value)",
            "Discard the error inside Err",
            "None",
        ),
        "err" => (
            "Discard the value inside Ok",
            "None",
            "Keep the error inside Err",
            "Some(error)",
        ),
        "unwrap_or_default" => (
            "Use the value inside Ok",
            "value",
            "Call Default::default() for the success type and leave the error unused",
            "default value",
        ),
        "map" => (
            "Run the callback with the value",
            "Ok(callback result)",
            "Carry the error through",
            "Err(error)",
        ),
        "map_err" => (
            "Carry the value through",
            "Ok(value)",
            "Run the callback with the error",
            "Err(callback result)",
        ),
        "and_then" => (
            "Run the callback with the value",
            "The callback's Result",
            "Carry the error through",
            "Err(error)",
        ),
        "or_else" => (
            "Carry the value through",
            "Ok(value)",
            "Run the recovery callback",
            "The callback's Result",
        ),
        "transpose" => {
            return vec![
                branch(
                    "Success · present",
                    "Move the success inside the Option",
                    "Some(Ok(value))",
                ),
                branch("Success · missing", "Return the absence", "None"),
                branch("Error", "Wrap the error as present", "Some(Err(error))"),
            ];
        }
        "unwrap_or" => (
            "Evaluate the default and keep the value",
            "value",
            "Evaluate and use the default",
            "default",
        ),
        "unwrap_or_else" => (
            "Keep the value and skip the fallback",
            "value",
            "Run the fallback with the error",
            "callback result",
        ),
        "unwrap" | "expect" => (
            "Take the successful value",
            "value",
            "Panic",
            "No value is returned",
        ),
        _ => return Vec::new(),
    };
    vec![
        branch("Success", success, output),
        branch("Error", error, failure),
    ]
}

fn operation_detail(family: Family, name: &str) -> Option<(&'static str, &'static str)> {
    Some(match (family, name) {
        (Family::Slice, "split_at_mut") => (
            "Split a mutable borrow",
            "Borrow the ranges before and from the split position. The ranges are disjoint. An out-of-bounds position panics.",
        ),
        (Family::Iter, "iter") => (
            "Read items by reference",
            "The collection stays with its owner. The iterator yields shared references.",
        ),
        (Family::Iter, "iter_mut") => (
            "Iterate with permission to change items",
            "The collection stays with its owner. The iterator yields exclusive references.",
        ),
        (Family::Iter, "into_iter") => (
            "Take the source by value",
            "An owned collection is consumed. Passing a reference iterates over borrowed items.",
        ),
        (Family::Iter, "find_map") => (
            "Find the first value produced by the callback",
            "Try the callback on items in order. Return the first Some result, or None if the iterator runs out of items first.",
        ),
        (Family::Iter, "filter_map") => (
            "Yield values produced by the callback",
            "When an item is requested, run the callback on candidate items. Yield the value inside Some and skip items that produce None.",
        ),
        (Family::Iter, "try_fold") => (
            "Accumulate until the callback stops",
            "Start with the initial accumulator. Each successful callback result supplies the next accumulator. A result that signals a stop, such as None or Err, stops the fold and becomes its result.",
        ),
        (Family::Iter | Family::ParIter, "map" | "filter" | "filter_map" | "zip" | "chain") => (
            "Prepare an iterator step",
            "This step is applied when items are requested or the iterator is consumed.",
        ),
        (Family::Iter | Family::ParIter, "collect" | "sum") => (
            "Consume the iterator",
            "Drive the preceding steps to produce a result.",
        ),
        (Family::ParIter, "par_iter" | "par_iter_mut" | "into_par_iter") => (
            "Prepare parallel iteration",
            "Items can be processed in parallel when a consumer drives the iterator.",
        ),
        (Family::Thread, "spawn") => (
            "Start a thread",
            "Run the function on a new thread and return a handle to it.",
        ),
        (Family::Thread, "join") => (
            "Wait for a thread",
            "Block this thread until the other thread finishes.",
        ),
        (Family::Task, "spawn") => (
            "Schedule a task",
            "Schedule the future on the runtime and return a task handle.",
        ),
        (Family::Effect, "send") => ("Send to a channel", "Pass the value to the channel. Inspect the result for delivery or an error."),
        (Family::Effect, "recv_optional") => ("Prepare to receive a message", "Await the future to receive a message. It returns None when the channel is closed and empty."),
        (Family::Effect, "recv_blocking") => (
            "Receive a message on this thread",
            "Return the next queued message as Ok(message). If the queue is empty and a sender still exists, block this thread until a message arrives or all senders disconnect. When no senders or queued messages remain, return Err(RecvError).",
        ),
        (Family::Effect, "recv") => ("Receive from a channel", "Use the next message or the error reported by the channel."),
        (Family::Effect, "deque_pop_front") => (
            "Remove the front item from the queue",
            "If the queue has an item, remove it from the front and return Some(item). An empty queue returns None.",
        ),
        (Family::Effect, "sleep") => ("Block the thread for a duration", "The calling thread waits for the duration."),
        (Family::Effect, "sleep_async") => ("Create a timer future", "Await the future to wait for the duration. The task can suspend."),
        (Family::Effect, "send_async") => ("Prepare to send a message", "Await the future to send the message when the channel has capacity."),
        (Family::Effect, "drop") => ("Drop the supplied value", "Pass the value to drop now. A resource is released when the owning value is dropped."),
        (Family::Effect, "exit") => ("Exit the process", "Stop the process with the supplied exit code."),
        (Family::Effect, "acquire") => ("Prepare to acquire a permit", "Await a permit. The returned permit normally releases its slot when it is dropped."),
        (Family::Arc, "clone") => (
            "Share ownership",
            "The new Arc handle shares the same allocation.",
        ),
        (Family::Lock, "lock" | "try_lock" | "read" | "write" | "try_read" | "try_write") => (
            "Acquire protected access",
            "On success, the returned guard holds the lock until the guard is dropped.",
        ),
        (Family::Cell, "borrow_mut") => (
            "Check the borrow at runtime",
            "The RefCell checks for conflicting borrows. If one exists, the call panics.",
        ),
        (Family::Clone, "clone") => (
            "Clone the value",
            "The type's Clone implementation decides how the new value is made.",
        ),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn vec_repeat_requires_two_complete_expressions() {
        for (arguments, expected) in [
            ("false; ships.len()", true),
            ("value::<u32, u64>(); count", true),
            ("{ seed(); 1 }; 0", true),
            (r#"";", { seed(); "ready" }"#, false),
            ("1, 2; count", false),
            ("1; count, other", false),
            ("1; count;", false),
            ("1;", false),
            ("; count", false),
            ("1; _", false),
        ] {
            let text = format!("fn f() {{ let _ = vec![{arguments}]; }}");
            let tree = crate::parse(&text).unwrap();
            let root = tree.root_node();
            let offset = text.find("vec!").unwrap();
            let name = root.descendant_for_byte_range(offset, offset + "vec".len()).unwrap();
            let call = std::iter::successors(Some(name), Node::parent).find(|node| node.kind() == "macro_invocation").unwrap();
            let index = LineIndex::new(&text);
            let src = Src { text: &text, file: "vec.rs", index: &index };
            let evidence = Evidence {
                operations: [(range(call), Some(Resolved { family: Family::Macro, name: "vec".into(), definition: None }))].into(),
                ..Default::default()
            };
            let (reading, _, _) = collect(src, root, &evidence);
            let found = reading.iter().any(|fact| matches!(&fact.content,
                ReadingContent::Steps { label, .. } if label == "Build a vector by repetition"));
            assert_eq!(found, expected, "{arguments}");
        }
    }

    #[test]
    fn select_direct_handler_uses_its_own_future_and_keyword_span() {
        let text = r#"async fn run() {
            loop {
                let mut selected = tokio::select! {
                    _ = ready(()) => |_: (), _: ()| x = y,
                    _ = ready(()) => break,
                    _ = ready(()) => continue
                };
                selected((), ());
            }
        }"#;
        let tree = crate::parse(text).unwrap();
        let root = tree.root_node();
        let offset = text.find("select!").unwrap();
        let name = root.descendant_for_byte_range(offset, offset + "select".len()).unwrap();
        let call = std::iter::successors(Some(name), Node::parent).find(|node| node.kind() == "macro_invocation").unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "select.rs", index: &index };
        let evidence = Evidence {
            operations: [(range(call), Some(Resolved { family: Family::Macro, name: "select".into(), definition: None }))].into(),
            ..Default::default()
        };
        let (reading, _, _) = collect(src, root, &evidence);
        let steps = reading.iter().find_map(|fact| match &fact.content {
            ReadingContent::Steps { steps, .. } => Some(steps),
            _ => None,
        }).unwrap();
        assert_eq!(steps.len(), 3, "{steps:?}");
        for (step, keyword, action) in [
            (&steps[1], "break", "Leave the enclosing loop"),
            (&steps[2], "continue", "Skip the rest of this iteration"),
        ] {
            assert_eq!(&text[step.span.start_byte as usize..step.span.end_byte as usize], keyword);
            assert!(step.action.starts_with(&format!("ready(()) → {action}")), "{step:?}");
        }
    }

    #[test]
    fn select_keeps_block_arms_with_or_without_commas() {
        let text = r#"fn run() {
            tokio::select! {
                biased;
                Some(value) = rx.recv(), if ready => { consume(value) }
                // There is no comma above.
                _ = wait() => { tick() },
                else => fallback(),
            }
        }"#;
        let tree = crate::parse(text).unwrap();
        let root = tree.root_node();
        let offset = text.find("select").unwrap();
        let name = root.descendant_for_byte_range(offset, offset + "select".len()).unwrap();
        let call = std::iter::successors(Some(name), Node::parent).find(|node| node.kind() == "macro_invocation").unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "select.rs", index: &index };
        let evidence = Evidence {
            operations: [(range(call), Some(Resolved { family: Family::Macro, name: "select".into(), definition: None }))].into(),
            ..Default::default()
        };
        let (reading, _, _) = collect(src, root, &evidence);
        let steps = reading.iter().find_map(|fact| match &fact.content {
            ReadingContent::Steps { steps, .. } => Some(steps),
            _ => None,
        }).unwrap();
        assert_eq!(steps.len(), 3, "{steps:?}");
        assert!(steps[0].action.contains("rx.recv(), if ready"));
        assert!(steps[1].action.contains("wait()"));
        assert_eq!(steps[2].action, "Run the else branch");
    }
}
