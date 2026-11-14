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
    syntax_sites::{rust_assignee_parts, rust_assignee_rest},
};
use tree_sitter::Node;

use crate::{expr::method_call, syntax::{block_statements, inclusive_range_to, statement_tail, unparen}};

type Range = (usize, usize);

pub(crate) fn contract(src: Src, node: Node) -> Option<Vec<ContractItem>> {
    let function = matches!(node.kind(), "function_item" | "function_signature_item");
    let member_owner = node.parent().and_then(|body| body.parent());
    let owner = member_owner.filter(|owner| owner.kind() == "impl_item");
    let mut items = Vec::new();
    items.extend(crate::const_explanation::explain(src, node));
    items.extend(crate::static_explanation::explain(src, node));
    items.extend(crate::static_mut_explanation::explain(src, node));
    items.extend(crate::function_explanation::explain(src, node));
    items.extend(crate::function_signature_explanation::explain(src, node));
    items.extend(crate::pub_function_explanation::explain(src, node));
    items.extend(crate::inline_module_explanation::explain(src, node));
    items.extend(crate::import_item_explanation::explain(src, node));
    items.extend(crate::import_path_explanation::explain(src, node));
    items.extend(crate::grouped_import_explanation::explain(src, node));
    items.extend(crate::import_alias_explanation::explain(src, node));
    items.extend(crate::public_reexport_explanation::explain(src, node));
    items.extend(crate::extern_crate_explanation::explain_module(src, node));
    items.extend(crate::public_visibility_explanation::explain(src, node));
    items.extend(crate::pub_crate_explanation::explain(src, node));
    items.extend(crate::pub_super_explanation::explain(src, node));
    items.extend(crate::pub_self_explanation::explain(src, node));
    items.extend(crate::pub_in_explanation::explain(src, node));
    items.extend(crate::crate_path_explanation::imports(src, node));
    items.extend(crate::self_path_explanation::imports(src, node));
    items.extend(crate::super_path_explanation::imports(src, node));
    items.extend(crate::self_type_explanation::explain(src, node));
    items.extend(crate::raw_identifier_explanation::explain(src, node));
    items.extend(crate::absolute_path_explanation::explain_imports(src, node));
    items.extend(crate::super_single_path_explanation::imports(src, node));
    items.extend(crate::self_explanation::receiver(src, node));
    items.extend(crate::named_struct_explanation::explain(src, node));
    items.extend(crate::tuple_struct_explanation::explain(src, node));
    items.extend(crate::newtype_explanation::explain(src, node));
    items.extend(crate::raw_lifetime_label_explanation::contract(src, node));
    items.extend(crate::unit_struct_explanation::explain(src, node));
    items.extend(crate::unit_enum_explanation::explain(src, node));
    items.extend(crate::enum_tuple_payload_explanation::explain(src, node));
    items.extend(crate::named_enum_variant_explanation::explain(src, node));
    items.extend(crate::union_declaration_explanation::explain(src, node));
    items.extend(crate::trait_declaration_explanation::explain(src, node));
    items.extend(crate::enum_discriminant_explanation::explain(src, node));
    items.extend(crate::supertrait_explanation::explain(src, node));
    items.extend(crate::multiple_supertraits_explanation::explain(src, node));
    items.extend(crate::shared_receiver_explanation::explain(src, node));
    items.extend(crate::new_associated_function_explanation::explain(src, node));
    items.extend(crate::blanket_impl_explanation::explain(src, node));
    items.extend(crate::trait_impl_explanation::explain(src, node));
    items.extend(crate::generic_inherent_impl_explanation::explain(src, node));
    items.extend(crate::inherent_impl_explanation::explain(src, node));
    items.extend(crate::associated_type_impl_explanation::explain(src, node));
    items.extend(crate::trait_associated_type_explanation::explain(src, node));
    items.extend(crate::default_trait_method_explanation::explain(src, node));
    items.extend(crate::unsafe_trait_explanation::explain(src, node));
    items.extend(crate::required_associated_const_explanation::explain(src, node));
    items.extend(crate::boxed_receiver_explanation::explain(src, node));
    items.extend(crate::arc_receiver_explanation::explain(src, node));
    items.extend(crate::unsafe_impl_explanation::explain(src, node));
    items.extend(crate::type_alias_explanation::explain(src, node));
    items.extend(crate::add_impl_explanation::explain(src, node));
    items.extend(crate::pointer_sized_integer_explanation::contract(src, node));
    items.extend(crate::str_type_explanation::contract(src, node));
    items.extend(crate::unit_type_explanation::contract(src, node));
    items.extend(crate::unsigned_integer_type_explanation::explain(src, node));
    items.extend(crate::tuple_type_explanation::contract(src, node));
    items.extend(crate::array_type_explanation::contract(src, node));
    items.extend(crate::single_tuple_type_explanation::contract(src, node));
    items.extend(crate::slice_type_explanation::contract(src, node));
    items.extend(crate::shared_reference_type_explanation::contract(src, node));
    items.extend(crate::shared_slice_reference_explanation::contract(src, node));
    items.extend(crate::mutable_reference_type_explanation::contract(src, node));
    items.extend(crate::mutable_slice_reference_explanation::contract(src, node));
    items.extend(crate::raw_const_pointer_explanation::contract(src, node));
    items.extend(crate::raw_mut_pointer_explanation::contract(src, node));
    items.extend(crate::integer_128_explanation::explain(src, node));
    items.extend(crate::signed_integer_explanation::explain(src, node));
    items.extend(crate::float_type_explanation::contract(src, node));
    items.extend(crate::char_type_explanation::contract(src, node));
    items.extend(crate::str_reference_explanation::contract(src, node));
    items.extend(crate::string_type_explanation::contract(src, node));
    let import_container = match node.kind() {
        "source_file" => Some(node),
        "mod_item" => node.child_by_field_name("body"),
        _ => None,
    };
    if let Some(container) = import_container {
        for import in named_children(container) {
            items.extend(crate::import_self_group_explanation::explain(src, import));
            items.extend(crate::glob_import_explanation::explain(src, import));
        }
    }
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
                "never_type" => crate::never_return_reading::explain(result).unwrap(),
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
    Ordering,
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
    OneShot,
    ArrayFromFn,
    IterFromFn,
    FutureReady,
    BoxPin,
    FuturesOrderedPush,
    StreamNext,
    BroadcastRecv,
    BroadcastSender,
    MemReplace,
    OnceLock,
    NonZero,
    StringInsert,
    Utf8,
    InclusiveRange,
    VecDrain,
    VecTruncate,
    VecRemove,
    VecExtend,
    VecRetain,
    StrSplit,
    Length,
    Peekable,
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

pub(crate) fn written_option_type(ty: Node<'_>, identities: &HashMap<Range, DefaultType>) -> bool {
    let mut ty = crate::syntax::unparen_type(ty);
    if ty.kind() == "reference_type" {
        let Some(inner) = ty.child_by_field_name("type") else { return false };
        ty = crate::syntax::unparen_type(inner);
    }
    ty.kind() == "generic_type" && ty.child_by_field_name("type").is_some_and(|head| {
        identities.get(&range(head)) == Some(&DefaultType::Option)
    })
}

pub(crate) fn written_result_type(ty: Node<'_>, identities: &HashMap<Range, DefaultType>) -> bool {
    let ty = crate::syntax::unparen_type(ty);
    ty.kind() == "generic_type" && ty.child_by_field_name("type").is_some_and(|head| {
        identities.get(&range(head)) == Some(&DefaultType::Result)
    })
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
    pub result_calls: HashSet<Range>,
    pub option_values: HashSet<Range>,
    pub ordered_queue_bindings: HashSet<Range>,
    pub registry_selects: HashSet<Range>,
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

fn call_member(call: Node<'_>) -> Option<Node<'_>> {
    if let Some((_, member)) = method_call(call) {
        return Some(member);
    }
    let callee = call.child_by_field_name("function")?;
    let callee = if callee.kind() == "generic_function" { callee.child_by_field_name("function")? } else { callee };
    Some(callee.child_by_field_name("name").unwrap_or(callee))
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

fn ordered_push_parts(call: Node<'_>) -> Option<(Node<'_>, Node<'_>)> {
    let (_, receiver_argument) = receiver(call)?;
    let member = method_call(call).map(|(_, member)| member).or_else(|| {
        call.child_by_field_name("function")?.child_by_field_name("name")
    })?;
    Some((member, argument(call, usize::from(receiver_argument.is_some()))?))
}

#[derive(Clone, Copy)]
enum IteratorCallback {
    Find,
    Position,
    Any,
    All,
    FindMap,
    FilterMap,
    Fold,
    TryFold,
}

impl IteratorCallback {
    fn operation(operation: &Resolved) -> Option<Self> {
        if operation.family != Family::Iter {
            return None;
        }
        match operation.name.as_str() {
            "find" => Some(Self::Find),
            "position" => Some(Self::Position),
            "any" => Some(Self::Any),
            "all" => Some(Self::All),
            "find_map" => Some(Self::FindMap),
            "filter_map" => Some(Self::FilterMap),
            "fold" => Some(Self::Fold),
            "try_fold" => Some(Self::TryFold),
            _ => None,
        }
    }

    fn is_callback(self, call: Node, closure: Node) -> bool {
        let Some((_, receiver_argument)) = receiver(call) else { return false };
        let index = usize::from(receiver_argument.is_some()) + usize::from(matches!(self, Self::Fold | Self::TryFold));
        argument(call, index) == Some(closure)
    }

    fn timing(self) -> &'static str {
        match self {
            Self::Find => "While find searches",
            Self::Position => "While position searches",
            Self::Any => "While any checks for a match",
            Self::All => "While all checks each item",
            Self::FindMap => "While find_map searches",
            Self::FilterMap => "When the iterator requests an item",
            Self::Fold => "While fold processes items",
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

enum Predicate {
    Retain,
    Search,
    Any,
    All,
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
                    } else if operation.family == Family::OnceLock && operation.name == "get_or_init" {
                        let (_, receiver_argument) = receiver(call)?;
                        (argument(call, usize::from(receiver_argument.is_some())) == Some(scope))
                            .then(|| "If this call runs the initializer".into())
                    } else if operation.family == Family::Option
                        && matches!(operation.name.as_str(), "map_or" | "map_or_else")
                    {
                        let (_, receiver_argument) = receiver(call)?;
                        let first = usize::from(receiver_argument.is_some());
                        if argument(call, first + 1) == Some(scope) {
                            Some("When the option contains a value".into())
                        } else if operation.name == "map_or_else" && argument(call, first) == Some(scope) {
                            Some("When the option is empty".into())
                        } else {
                            None
                        }
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
        if let Some((comma, content)) = crate::single_tuple_type_explanation::local(self.src, node) { self.push(self.src.span(node), self.src.span(comma), false, content); }
        if let Some((highlight, content)) = crate::float_type_explanation::local(self.src, node) { self.push(self.src.span(node), self.src.span(highlight), false, content); }
        for (ty, content) in crate::char_type_explanation::local(self.src, node) {
            let span = if node.child_by_field_name("type") == Some(ty) { children(node).into_iter().find(|part| part.kind() == ":").map_or_else(|| self.src.span(ty), |part| self.src.span(part)) } else { self.src.span(ty) };
            self.push(self.src.span(node), span, false, content);
        }
        if let Some(content) = crate::pointer_sized_integer_explanation::reading(self.src, node) {
            if let Some(binding) = node.parent().filter(|parent| parent.kind() == "let_declaration" && parent.child_by_field_name("type") == Some(node)) {
                let span = children(binding).into_iter().find(|part| part.kind() == ":").map_or_else(|| self.src.span(node), |part| self.src.span(part));
                self.push(self.src.span(binding), span, false, content);
            } else { self.at(node, false, content); }
        }
        if let Some(content) = crate::str_type_explanation::reading(self.src, node) { self.at(node, false, content); }
        if let Some(content) = crate::raw_lifetime_label_explanation::label(self.src, node) { self.at(node, false, content); }
        if let Some((span, content)) = crate::absolute_path_explanation::explain_path(self.src, node) {
            self.push(self.src.span(node), span, false, content);
        }
        match node.kind() {
            "unit_expression" => { if let Some(content) = crate::unit_type_explanation::value(node) { self.at(node, false, content); } }
            "scoped_identifier" | "scoped_type_identifier" => {
                let content = crate::crate_path_explanation::expression(self.src, node)
                    .or_else(|| crate::self_path_explanation::expression(self.src, node))
                    .or_else(|| crate::super_path_explanation::expression(self.src, node))
                    .or_else(|| crate::super_single_path_explanation::expression(self.src, node))
                    .or_else(|| crate::path_separator_explanation::explain(self.src, node).map(|text| detail(crate::path_separator_explanation::TITLE, text)));
                if let Some(content) = content { self.at(node, false, content); }
            }
            "call_expression" => self.call(node),
            "try_expression" => self.propagation(node),
            "type_cast_expression" => {
                if let Some(content) = crate::cast_as_explanation::explain(self.src, node) { self.at(node, false, content); }
                self.numeric_cast(node);
            }
            "array_expression" => self.array_repeat(node),
            "struct_expression" => {
                if let Some((field, text)) = crate::union_initialization_explanation::explain(self.src, node) { self.push(self.src.span(node), self.src.span(field), false, detail("Initialize a union field", text)); }
                if let Some((path, content)) = crate::enum_named_construction_explanation::explain(self.src, node) { self.push(self.src.span(node), self.src.span(path), false, content); }
                if let Some(content) = crate::struct_shorthand_reading::explain(self.src, node).or_else(|| crate::record_construct_explanation::explain(self.src, node)) { self.at(node, false, content); }
            }
            "range_expression" => self.range_expression(node),
            "assignment_expression" => {
                if let Some((equal, end)) = inclusive_range_to(node) {
                    let operator = self.src.index.span(self.src.file, node.start_byte(), equal.end_byte());
                    self.describe_range(node, operator, None, Some(end), true);
                } else {
                    self.destructuring_assignment(node);
                }
            }
            "binary_expression" => self.short_circuit(node),
            "let_chain" => self.let_chain(node),
            "loop_expression" => self.bare_loop(node),
            "while_expression" => self.while_let(node),
            "closure_expression" => self.closure(node),
            "return_expression" => self.predicate_return(node),
            "block" => self.block_result(node),
            "base_field_initializer" => self.struct_update(node),
            "macro_invocation" => self.macro_call(node),
            "await_expression" => {
                if self.op(node).is_some_and(|op| op.family == Family::OneShot && op.name == "poll") {
                    if let Some(keyword) = children(node).into_iter().find(|token| token.kind() == "await") {
                        self.oneshot_receive(node, self.src.span(keyword), false);
                    }
                } else {
                    self.at(node, false, detail("Await a result", "Continue here when the future is ready. If it is pending, this task can suspend."));
                }
            }
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
        if rust_assignee_rest(node) || node.parent().is_some_and(|parent| {
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

    fn destructuring_assignment(&mut self, node: Node<'_>) {
        if node.has_error() {
            return;
        }
        let (Some(mut left), Some(right)) = (node.child_by_field_name("left"), node.child_by_field_name("right")) else { return };
        let Some(operator) = children(node).into_iter().find(|token| token.kind() == "=" && !token.is_missing()) else { return };
        while left.kind() == "parenthesized_expression" {
            let Some(inner) = left.named_children(&mut left.walk()).find(|child| !child.is_extra()) else { return };
            left = inner;
        }
        if !matches!(left.kind(), "tuple_expression" | "struct_expression" | "array_expression") {
            return;
        }
        let mut steps = vec![self.step(right, "First", "Evaluate the right side and take the corresponding parts for the destinations. If evaluation exits early, this assignment never starts writing to the destinations.")];
        let mut pending = vec![left];
        while let Some(part) = pending.pop() {
            let step = match part.kind() {
                "array_expression" if part.child_by_field_name("length").is_some() => return,
                "struct_expression" if part.child_by_field_name("body")
                    .is_some_and(|body| children(body).iter().any(|field| field.kind() == "base_field_initializer")) => return,
                "parenthesized_expression" | "tuple_expression" | "struct_expression" | "array_expression" => {
                    pending.extend(rust_assignee_parts(part).into_iter().rev());
                    continue;
                }
                "identifier" if self.src.text(part) == "_" => self.step(part, "Ignore one part", "Skip the value at this position. There is no destination to write."),
                "range_expression" if rust_assignee_rest(part) => self.step(part, "Ignore the remaining parts", "The dots skip the elements not named by the other destinations."),
                "unary_expression" if part.child(0).is_none_or(|token| token.kind() != "*") => return,
                "identifier" | "scoped_identifier" | "field_expression" | "index_expression" | "unary_expression" => {
                    let destination = self.written(part, 80);
                    self.step(part, "In left-to-right order", format!("Locate {destination}, drop its previous value if initialized, and write the corresponding part by move or copy."))
                }
                _ => return,
            };
            steps.push(step);
        }
        if steps.len() == 1 {
            return;
        }
        self.push(self.src.span(node), self.src.span(operator), false, ReadingContent::Steps {
            label: "Assign parts to existing destinations".into(), steps,
        });
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

    fn bare_loop(&mut self, node: Node<'_>) {
        let Some(body) = node.child_by_field_name("body") else { return };
        let Some(keyword) = children(node).into_iter().find(|token| token.kind() == "loop" && !token.is_missing()) else { return };
        self.push(
            self.src.header_span(node, Some(body)),
            self.src.span(keyword),
            false,
            detail(
                "Repeat the loop body",
                "Start the body without checking a condition. If the body reaches its end, start it again. A continue targeting this loop skips the rest of this iteration and goes back to the start. A break targeting this loop ends it, and break with a value supplies the result of the loop expression. A return can leave the enclosing function, closure, or async block before the loop produces a value.",
            ),
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
                    .is_some_and(|op| op.family == Family::Effect && matches!(op.name.as_str(), "recv_optional" | "recv_mpsc"));
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
        let discarded = expression_result_is_discarded(node);
        if discarded {
            branches[0].action.push_str(" and ignore the returned value");
            branches[0].output = "Continue after this statement".into();
        }
        if let Some((_, output)) = iterator_exit {
            if !discarded {
                branches[0].output = "Continue this callback with the value".into();
            }
            branches[1].output = output.into();
        } else {
            if let Some(resume) = question_mark_condition(node).map(str::to_owned)
                .or_else(|| question_mark_consumer(self.src, node))
            {
                branches[0].output = resume;
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
        if let Some((arguments, text)) = crate::tuple_variant_call_explanation::explain(self.src, node) { self.push(self.src.span(node), self.src.span(arguments), false, detail("Construct a tuple-style enum variant", text)); }
        if let Some((callee, text)) = crate::tuple_struct_call_explanation::explain(self.src, node) {
            self.push(self.src.span(node), self.src.span(callee), false, detail("Construct a tuple struct", text));
        }
        self.collect_type_arguments(node);
        let Some(op) = self.op(node) else { return };
        let family = op.family;
        let name = op.name.clone();
        match family {
            Family::Bool => self.bool_call(node, &name),
            Family::Ordering => self.ordering_call(node, &name),
            Family::OneShot => self.oneshot_call(node, &name),
            Family::Option | Family::Result => self.container_call(node, family, &name),
            Family::Arc if matches!(name.as_str(), "make_mut" | "unwrap_or_clone") => self.arc_value(node, &name),
            Family::Arc if name == "try_unwrap" => self.arc_try_unwrap(node),
            Family::Lock if name == "mutex_into_inner" => self.mutex_into_inner(node),
            Family::Peekable => self.peekable(node, &name),
            Family::Iter if name == "zip" => self.iterator_zip(node),
            Family::Iter if name == "fold" => self.iterator_fold(node),
            Family::ArrayFromFn | Family::IterFromFn => self.from_fn(node, family),
            Family::FutureReady | Family::BoxPin => self.future_preparation(node, family),
            Family::FuturesOrderedPush => self.ordered_push(node),
            Family::MemReplace => self.mem_replace(node, &name),
            Family::OnceLock => self.once_lock_call(node, &name),
            Family::Length => {
                self.inferred(node, &TypeInfo::new(TypeShape::scalar("UIntSize"), TypeOrigin::Compiler, Some("usize".into())));
                let (label, text) = if name == "bytes" {
                    ("Count UTF-8 bytes", "Return the number of UTF-8 bytes in the text as a usize. For example, the character 'é' occupies two bytes.")
                } else {
                    ("Count elements", "Return the number of elements currently in this vector or slice as a usize. Each element counts once, regardless of its size in bytes.")
                };
                self.at(node, true, detail(label, text));
            }
            Family::StringInsert => self.string_insert(node),
            Family::Utf8 => self.utf8(node, &name),
            Family::InclusiveRange => self.inclusive_range_contains(node),
            Family::VecDrain => self.vec_drain(node),
            Family::VecTruncate => self.vec_truncate(node),
            Family::VecRemove if name == "dedup" => self.vec_dedup(node),
            Family::VecRemove => self.vec_remove(node, &name),
            Family::VecExtend => self.vec_extend(node, name == "copy"),
            Family::VecRetain => self.vec_retain(node, &name),
            Family::StrSplit => self.str_split(node, &name),
            Family::NumericFrom { source, target } | Family::NumericTryFrom { source, target } => {
                self.numeric_conversion(node, source, target, matches!(family, Family::NumericTryFrom { .. }));
            }
            Family::Map | Family::Slice if matches!(name.as_str(), "get" | "get_mut") => {
                self.get(node)
            }
            Family::Slice if name == "get_disjoint_mut" => self.disjoint_mut(node),
            Family::Slice if matches!(name.as_str(), "sort_by" | "sort_by_key" | "sort_unstable_by") => self.sort_slice(node, &name),
            Family::Slice if name == "select_nth_unstable_by" => self.select_nth_unstable_by(node),
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
            Family::Effect if matches!(name.as_str(), "timeout" | "timeout_at") => self.timeout(node, &name),
            Family::BroadcastRecv => self.broadcast_receive(node),
            Family::BroadcastSender => self.broadcast_sender(node, &name),
            Family::Iter | Family::ParIter if matches!(name.as_str(), "collect" | "sum" | "count" | "for_each") => {
                self.sequence(node, &name);
            }
            Family::Iter if matches!(name.as_str(), "find" | "position" | "any" | "all") => self.iterator_search(node, &name),
            _ => {
                if let Some((label, description)) = operation_detail(family, &name) {
                    self.at(node, true, detail(label, description));
                }
            }
        }
    }

    fn oneshot_call(&mut self, node: Node<'_>, name: &str) {
        let Some(member) = call_member(node) else { return };
        let span = self.src.span(member);
        match name {
            "channel" => self.push(self.src.span(node), span, true, detail(
                "Create a channel for one value",
                "Return (sender, receiver). The sender can send one value, and the receiver can wait for it. Creating this pair does not send a value or start a task.",
            )),
            "send" => {
                let Some((_, receiver_argument)) = receiver(node) else { return };
                let Some(value) = argument(node, usize::from(receiver_argument.is_some())) else { return };
                self.paths(node, span, true, format!("Use up this sender to send {} once. This call does not wait for the receiver.", self.written(value, 100)), vec![
                    branch("The channel accepts the value", "The receiver can take the value later. This does not confirm that it has read it.", "Ok(())"),
                    branch("The receiver has closed or been dropped", "Return the original value to the caller.", "Err(value)"),
                ], self.known_type(node));
            }
            "blocking_recv" => self.oneshot_receive(node, span, true),
            _ => {}
        }
    }

    fn oneshot_receive(&mut self, node: Node<'_>, span: Span, blocking: bool) {
        let input = if blocking {
            "Receive the channel's single value, blocking the current thread if it is still pending. This call panics inside an asynchronous execution context."
        } else {
            "The receiver is itself a future. Await its single value. If the channel is still pending, this task can suspend."
        };
        self.paths(node, span, true, input.into(), vec![
            branch("The channel delivers a value", "Use the value the sender sent, including false or an error if that is the value.", "Ok(value)"),
            branch("The channel closes without delivering a value", "The receiver completes with an error because there is no message to read.", "Err(RecvError)"),
        ], self.known_type(node));
    }

    fn broadcast_sender(&mut self, call: Node<'_>, name: &str) {
        let Some(member) = call_member(call) else { return };
        let Some((sender, receiver_argument)) = receiver(call) else { return };
        let span = self.src.span(member);
        if name == "subscribe" {
            self.push(self.src.span(call), span, true, detail(
                "Subscribe to future broadcasts",
                format!("Create a receiver for {}. It starts at the next send and receives a clone of each retained message when it reads it. Messages sent before this call are outside the subscription.", self.written(sender, 90)),
            ));
            return;
        }
        let Some(value) = argument(call, usize::from(receiver_argument.is_some())) else { return };
        self.paths(call, span, true,
            format!("Send {} through {}. This call returns without waiting for receivers to read the message.", self.written(value, 90), self.written(sender, 90)), vec![
                branch("At least one receiver is active", "Put the message in the shared buffer and report how many receivers were active. A receiver may drop or fall behind before reading it.", "Ok(receiver count)"),
                branch("No receivers are active", "Return a send error containing the original value.", "Err(SendError(value))"),
            ], self.known_type(call));
    }

    fn broadcast_receive(&mut self, call: Node<'_>) {
        let mut operand = call;
        while let Some(group) = operand.parent().filter(|parent| parent.kind() == "parenthesized_expression") {
            operand = group;
        }
        let Some(wait) = operand.parent().filter(|parent| parent.kind() == "await_expression") else {
            self.at(call, true, detail("Prepare to receive a broadcast", "Create a future for this receiver's next message. The receive runs when the future is polled."));
            return;
        };
        self.broadcast_receive_paths(wait, self.src.span(call), self.written(call, 80), false);
    }

    fn broadcast_receive_paths(&mut self, anchor: Node<'_>, call: Span, written: String, in_select: bool) {
        let start = if in_select { "When this branch is enabled and polled, wait" } else { "Wait" };
        let mut input = format!("{start} for this broadcast receiver through {written}. If there is no unread message and senders remain, the receive can stay pending.");
        if in_select {
            input.push_str(" If this receive is still pending when another branch wins, canceling this wait consumes no message.");
        }
        self.paths(anchor, call, true, input, vec![
            branch("Message received", "Read the next message for this receiver.", "Ok(message)"),
            branch("Receiver lagged", "The channel overwrote n messages before this receiver read them. Move this receiver's read position to the oldest message still retained.", "Err(RecvError::Lagged(n))"),
            branch("Channel closed", "All senders are gone and this receiver has no buffered messages left to read.", "Err(RecvError::Closed)"),
        ], None);
    }

    fn timeout(&mut self, call: Node<'_>, name: &str) {
        let mut operand = call;
        while let Some(group) = operand.parent().filter(|parent| parent.kind() == "parenthesized_expression") {
            operand = group;
        }
        let Some(wait) = operand.parent().filter(|parent| parent.kind() == "await_expression") else { return };
        let (Some(limit), Some(future)) = (argument(call, 0), argument(call, 1)) else { return };
        let timing = if name == "timeout_at" {
            format!("Set the deadline to {} when creating the timeout.", self.written(limit, 80))
        } else {
            format!("Start the {} time limit when creating the timeout.", self.written(limit, 80))
        };
        let (goal, mut branches) = match self.op(future).filter(|op| op.family == Family::Effect).map(|op| op.name.as_str()) {
            Some("recv_websocket") => ("Wait for a WebSocket message or the deadline", vec![
                branch("Message received", "Use the message reported by the socket, which can be a close frame.", "Ok(Some(Ok(message)))"),
                branch("Socket closed", "The receive completes without another message.", "Ok(None)"),
                branch("Receive failed", "Keep the receive error inside the completed timeout result.", "Ok(Some(Err(error)))"),
            ]),
            Some("recv_mpsc") => ("Wait for a message or the deadline", vec![
                branch("Message received", "Take the message from the channel.", "Ok(Some(message))"),
                branch("Channel closed", "The closed channel has no messages left.", "Ok(None)"),
            ]),
            _ => ("Wait for the future or the deadline", vec![
                branch("Future completes", "Wrap the future's original output in Ok. That output can itself be an error.", "Ok(output)"),
            ]),
        };
        let input = format!("{goal}. {timing} Tokio checks the wrapped future first, so a ready result wins even at an expired deadline. While waiting, the task can suspend. The timeout cannot interrupt work during a poll, and work started elsewhere may continue.");
        branches.push(branch("Deadline elapsed", "If the future is still pending when Tokio observes the expired timer, stop waiting.", "Err(Elapsed)"));
        self.paths(wait, self.src.span(call), true, input, branches, None);
    }

    fn sort_slice(&mut self, node: Node<'_>, name: &str) {
        let Some((slice, receiver_argument)) = receiver(node) else { return };
        let Some(compare) = argument(node, usize::from(receiver_argument.is_some())) else { return };
        let Some(member) = call_member(node) else { return };
        let (label, compare_action, equal, order_requirement) = if name == "sort_by_key" {
            (
                "Sort the original slice by keys",
                "Call this function with a shared reference to an element and use its returned key to order the elements. The sort may call it more than once for the same element.",
                "Elements with equal keys keep their original relative order.",
                "The keys must define a consistent total order.",
            )
        } else {
            let equal = if name == "sort_by" {
                "Elements that compare Equal keep their original relative order."
            } else {
                "Elements that compare Equal may change their relative order."
            };
            (
                "Sort the original slice with a comparison function",
                "Call this comparison function with shared references to two elements. Less places the first before the second. Greater places it after the second. Equal ties them.",
                equal,
                "The comparison must define a consistent total order.",
            )
        };
        let steps = vec![
            self.step(compare, "During the sort", compare_action),
            self.step(slice, "Reorder the elements", format!("Reorder the original slice. {equal} {order_requirement} An inconsistent order may cause a panic or leave the resulting order unspecified.")),
            self.step(member, "On normal completion", "Return (). The caller can read the new order from the original collection."),
        ];
        self.push(self.src.span(node), self.src.span(member), true, ReadingContent::Steps {
            label: label.into(),
            steps,
        });
    }

    fn disjoint_mut(&mut self, node: Node<'_>) {
        let Some((slice, receiver_argument)) = receiver(node) else { return };
        let Some(indices) = argument(node, usize::from(receiver_argument.is_some())) else { return };
        let input = format!(
            "Check that each index or range in {} is within {} and that the requested parts do not overlap.",
            self.written(indices, 100),
            self.written(slice, 80),
        );
        self.paths(node, self.src.span(node), true, input, vec![
            branch("All parts are valid and disjoint", "Return one mutable reference per requested part in the order supplied. Writing through a reference updates the original element or slice.", "Ok([&mut …])"),
            branch("An index or range is out of bounds", "Return an error without handing out mutable references.", "Err(IndexOutOfBounds)"),
            branch("Requested parts overlap", "Return an error without handing out mutable references.", "Err(OverlappingIndices)"),
        ], None);
    }

    fn peekable(&mut self, node: Node<'_>, name: &str) {
        let Some((iterator, receiver_argument)) = receiver(node) else { return };
        let iterator = self.written(iterator, 80);
        let (input, branches) = if name == "peek" {
            (
                format!("Look at the next item of {iterator}. Peekable may pull an item from the underlying iterator and store it so it stays next."),
                vec![
                    branch("An item is available", "Borrow the next item without consuming it. If the item is itself a reference, this returns a reference to that reference.", "Some(&item)"),
                    branch("No item this time", "There is no next item to borrow on this call", "None"),
                ],
            )
        } else {
            let Some(predicate) = argument(node, usize::from(receiver_argument.is_some())) else { return };
            let predicate = self.written(predicate, 120);
            (
                format!("Check only the next item of {iterator}. If an item is available, pass a reference to it to the predicate {predicate}."),
                vec![
                    branch("Predicate returns true", "Consume and return the next item", "Some(item)"),
                    branch("Predicate returns false", "Keep this item as the next one for a later call", "None"),
                    branch("No item this time", "Skip calling the predicate", "None"),
                ],
            )
        };
        self.paths(node, self.src.span(node), true, input, branches, self.known_type(node));
    }

    fn arc_try_unwrap(&mut self, node: Node<'_>) {
        let Some(pointer) = argument(node, 0) else { return };
        self.paths(node, self.src.span(call_member(node).unwrap_or(node)), true,
            format!("Take ownership of the Arc at {} and try to extract its value.", self.written(pointer, 120)), vec![
                branch("This Arc is the only strong owner", "Move the value out without cloning. Weak references may still exist, but can no longer be upgraded.", "Ok(value)"),
                branch("Another strong owner exists during the attempt", "Return the input Arc so it can still be used.", "Err(original Arc)"),
            ], self.known_type(node));
    }

    fn mutex_into_inner(&mut self, node: Node<'_>) {
        let Some((receiver, _)) = receiver(node) else { return };
        self.paths(node, self.src.span(call_member(node).unwrap_or(node)), true,
            format!("Consume {} and extract the protected value without acquiring the lock.", self.written(receiver, 120)), vec![
                branch("The mutex is not poisoned", "Move the protected value to the caller.", "Ok(value)"),
                branch("The mutex is poisoned", "Return the protected value inside PoisonError. Calling into_inner() on that error recovers the value.", "Err(PoisonError containing the value)"),
            ], self.known_type(node));
    }

    fn arc_value(&mut self, node: Node<'_>, name: &str) {
        let Some(pointer) = argument(node, 0) else { return };
        let pointer = self.written(pointer, 120);
        let (input, branches) = if name == "make_mut" {
            (
                format!("Get mutable access to the value inside the Arc at {pointer}."),
                vec![
                    branch("Only owner, no Weak pointers", "Reuse the existing value in its allocation", "Mutable reference to the value"),
                    branch("Other Arc owners", "Use the inner type's cloning behavior to create a value in a new allocation. If cloning finishes, point this Arc to it.", "Mutable reference to the clone"),
                    branch("Only owner, with Weak pointers", "Keep the inner value without cloning it. Existing Weak pointers can no longer be upgraded to an Arc.", "Mutable reference to the value"),
                ],
            )
        } else {
            (
                format!("Consume {pointer} and return an owned value."),
                vec![
                    branch("Only Arc owner", "Move the original value out of the Arc. Weak pointers do not prevent this.", "Original value"),
                    branch("Other Arc owners", "Use the inner type's Clone implementation and return its result if cloning finishes.", "Cloned value"),
                ],
            )
        };
        self.paths(node, self.src.span(node), true, input, branches, self.known_type(node));
    }

    fn str_split(&mut self, node: Node<'_>, name: &str) {
        let Some((text, receiver_argument)) = receiver(node) else { return };
        let Some(pattern) = argument(node, usize::from(receiver_argument.is_some())) else { return };
        let text = self.written(text, 100);
        let pattern = self.written(pattern, 80);
        if name == "split" {
            self.at(node, true, detail("Iterate over pieces of text", format!(
                "As the iterator is consumed, yield slices of {text} between matches for {pattern}. Each piece is a borrowed &str. Matches at the ends or next to each other can yield empty pieces. If the pattern never matches, the entire text is the only piece."
            )));
        } else {
            self.paths(node, self.src.span(node), true, format!("Look for the first match for {pattern} in {text}."), vec![
                branch("Pattern matches", "Borrow the text before and after the first match as two &str slices. Either slice can be empty.", "Some((before, after))"),
                branch("No match", "There is no pair of slices", "None"),
            ], self.known_type(node));
        }
    }

    fn vec_retain(&mut self, node: Node<'_>, name: &str) {
        let Some((receiver, position)) = receiver(node) else { return };
        let Some(test) = argument(node, usize::from(position.is_some())) else { return };
        let access = if name == "retain_mut" {
            "Give the test a mutable reference (&mut T) so it can change the element."
        } else {
            "Give the test a shared reference (&T) to the element."
        };
        self.steps(node, "Keep selected elements in the vector", vec![
            self.step(test, "During the call", format!("Call the test once per element in its original order. {access}")),
            self.step(receiver, "For each result", "Keep the element on true and remove it on false."),
            self.step(node, "If the call completes", "The kept elements stay in their original order in the same vector."),
        ]);
    }

    fn predicate_callback(&self, closure: Node<'_>) -> Option<Predicate> {
        let call = direct_argument_call(closure)?;
        let (_, position) = receiver(call)?;
        if argument(call, usize::from(position.is_some())) != Some(closure) {
            return None;
        }
        let operation = self.op(call)?;
        match (operation.family, operation.name.as_str()) {
            (Family::VecRetain, _) => Some(Predicate::Retain),
            (Family::Iter, "find" | "position") => Some(Predicate::Search),
            (Family::Iter, "any") => Some(Predicate::Any),
            (Family::Iter, "all") => Some(Predicate::All),
            _ => None,
        }
    }

    fn predicate_return(&mut self, node: Node<'_>) {
        let Some(closure) = boundary(node).filter(|scope| scope.kind() == "closure_expression") else { return };
        let Some(predicate) = self.predicate_callback(closure) else { return };
        let Some(value) = named_children(node).into_iter().find(|child| !child.is_extra()) else { return };
        self.predicate_decision(unparen(value), node, predicate);
    }

    fn predicate_decision(&mut self, value: Node<'_>, anchor: Node<'_>, predicate: Predicate) {
        if value.kind() != "boolean_literal" {
            return;
        }
        let (label, message) = match (predicate, self.src.text(value)) {
            (Predicate::Retain, "true") => ("Keep this element", "This result tells the vector to keep the current element."),
            (Predicate::Retain, "false") => ("Remove this element", "This result tells the vector to remove the current element."),
            (Predicate::Search, "true") => ("Accept this item and stop", "The search found its first matching item."),
            (Predicate::Search, "false") => ("Skip this item and keep looking", "The search can try another item if one is available."),
            (Predicate::Any, "true") => ("Stop and return true from any", "This item matches, which is enough for any to return true."),
            (Predicate::Any, "false") => ("Keep looking for a match", "This item fails the test. any checks the next item if one is available."),
            (Predicate::All, "true") => ("Keep checking the other items", "This item passes the test. all checks the next item if one is available."),
            (Predicate::All, "false") => ("Stop and return false from all", "This item fails the test, which is enough for all to return false."),
            _ => return,
        };
        self.push(self.src.span(anchor), self.src.span(value), true, detail(label, message));
    }

    fn iterator_search(&mut self, node: Node<'_>, name: &str) {
        let member = method_call(node).map(|(_, member)| member).or_else(|| {
            let callee = node.child_by_field_name("function")?;
            let callee = if callee.kind() == "generic_function" { callee.child_by_field_name("function")? } else { callee };
            callee.child_by_field_name("name")
        }).unwrap_or(node);
        if matches!(name, "any" | "all") {
            let (goal, decisive, fallback, stop, resume) = if name == "any" {
                ("Check whether at least one item passes the test.", "true", "false", "The test returns true", "Every tested item failed, or the iterator yielded no items.")
            } else {
                ("Check whether every item passes the test.", "false", "true", "The test returns false", "Every tested item passed, or the iterator yielded no items.")
            };
            let input = format!("{goal} During this call, the test receives each item by value as Item, which can itself be a reference. When the test returns {fallback}, try the next item if one is available.");
            self.paths(node, self.src.span(member), true, input, vec![
                branch(stop, "Stop at the first such item, after consuming it. The next read from the iterator can pick up after it.", decisive),
                branch("No item gave a decisive result", resume, fallback),
            ], self.known_type(node));
            return;
        }
        let (input, result) = if name == "find" {
            ("Find the first matching item. During this call, the test receives a shared reference (&Item) to each candidate. If the iterator yields references, find gives the test a reference to that reference.", "Some(item)")
        } else {
            ("Find the position of the first matching item. During this call, the test receives each candidate as Item, which can itself be a reference. Count from zero at the next item this iterator would yield.", "Some(index)")
        };
        let input = format!("{input} Skip each candidate for which the test returns false.");
        self.paths(node, self.src.span(member), true, input, vec![
            branch("The test returns true", "Stop after consuming the matching item. The next read from the iterator can pick up after it.", result),
            branch("No match in this search", "The iterator reaches a point where it yields no item. The test returned false for every item it saw.", "None"),
        ], self.known_type(node));
    }

    fn select_nth_unstable_by(&mut self, node: Node<'_>) {
        let Some((slice, position)) = receiver(node) else { return };
        let first = usize::from(position.is_some());
        let (Some(index), Some(compare)) = (argument(node, first), argument(node, first + 1)) else { return };
        let slice = self.written(slice, 80);
        let index_text = self.written(index, 80);
        let result = if expression_result_is_discarded(node) {
            "This statement ignores the three mutable references. The rearrangement remains in the original slice."
        } else {
            "Return three disjoint mutable references into the same slice: the prefix, the element at the index, and the suffix."
        };
        let steps = vec![
            self.step(index, "Index", format!("Choose position {index_text}, counting from zero. An index at or beyond the length panics, including on an empty slice.")),
            self.step(compare, "Rearrange in place", format!("Use the comparator to rearrange {slice}. On success, the element at the index occupies its position in the comparator's sorted order. The comparator must define a total order. An inconsistent comparator can cause a panic.")),
            self.step(node, "After rearranging", "The prefix compares less than or equal to the selected element. The suffix compares greater than or equal to it. Neither side has a guaranteed internal order, and ties can change relative order."),
            self.step(node, "Returned value", result),
        ];
        self.push(self.src.span(node), self.src.span(call_member(node).unwrap_or(node)), true,
            ReadingContent::Steps { label: "Select a position and partition the slice".into(), steps });
    }

    fn vec_extend(&mut self, node: Node<'_>, copy: bool) {
        let Some((vector, position)) = receiver(node) else { return };
        let Some(source) = argument(node, usize::from(position.is_some())) else { return };
        let vector = self.written(vector, 80);
        let source_value = if source.kind() == "reference_expression" {
            source.child_by_field_name("value").map(unparen).unwrap_or(source)
        } else {
            source
        };
        let option = self.evidence.option_values.contains(&range(source_value))
            || self.op(source_value).is_some_and(|op| op.family == Family::Option && op.name == "Some")
            || self.direct_option_receiver(source_value).is_some();
        let input = if option {
            "Use the Option as a source of zero or one item. None yields no item. Some supplies its value, or a reference to it if the Option is borrowed."
        } else if self.op(source).is_some_and(|op| op.family == Family::Iter && op.name == "map") {
            "Drive the input iterator now. When the mapped iterator supplies an item, its callback runs during this call."
        } else {
            "Use IntoIterator to turn the source into items and read them during this call."
        };
        let append = if copy {
            format!("Copy each referenced value and append the copy to the end of {vector} in the source's order.")
        } else {
            format!("Append each yielded item to the end of {vector} in the source's order.")
        };
        let steps = vec![
            self.step(source, "Read the source", input),
            self.step(node, "Extend the existing vector", append),
            self.step(node, "If the call completes", "The call returns () (unit)."),
        ];
        self.push(self.src.span(node), self.src.span(call_member(node).unwrap_or(node)), true,
            ReadingContent::Steps { label: "Append items to the vector".into(), steps });
    }

    fn vec_remove(&mut self, node: Node<'_>, method: &str) {
        let Some((vector, position)) = receiver(node) else { return };
        let Some(index) = argument(node, usize::from(position.is_some())) else { return };
        let vector = self.written(vector, 80);
        let index_text = self.written(index, 80);
        let label = if method == "swap_remove" { "Remove an element by swapping in the last one" } else { "Remove an element and preserve the order" };
        let mut steps = vec![self.step(index, "Index", format!("Select index {index_text} in {vector}, counting from zero. An index outside the vector panics."))];
        if method == "swap_remove" {
            steps.extend([
                self.step(node, "If the index is before the last element", "Move the last element into the removed element's position and shorten the vector. This can change the order."),
                self.step(node, "If the index is the last element", "Shorten the vector by one. No other element needs to move."),
            ]);
        } else {
            steps.push(self.step(node, "For a valid index", "Shift the elements after the removed one left by one slot and shorten the vector. The remaining elements keep their relative order."));
        }
        steps.push(self.step(node, "Returned value", "Return the removed element by value."));
        self.push(self.src.span(node), self.src.span(call_member(node).unwrap_or(node)), true,
            ReadingContent::Steps { label: label.into(), steps });
    }

    fn vec_dedup(&mut self, node: Node<'_>) {
        let Some((vector, _)) = receiver(node) else { return };
        let vector = self.written(vector, 80);
        let steps = vec![
            self.step(node, "During the call", format!("Compare consecutive elements of {vector} using the element type's PartialEq equality. Keep the first element in each run of equal neighbors and drop the rest in place.")),
            self.step(node, "If the call completes", "Equal elements separated by different values can both remain. The call returns () (unit)."),
        ];
        self.push(self.src.span(node), self.src.span(call_member(node).unwrap_or(node)), true,
            ReadingContent::Steps { label: "Remove consecutive duplicates".into(), steps });
    }

    fn vec_truncate(&mut self, node: Node<'_>) {
        let Some((vector, position)) = receiver(node) else { return };
        let Some(length) = argument(node, usize::from(position.is_some())) else { return };
        let vector = self.written(vector, 80);
        let length = self.written(length, 80);
        self.paths(node, self.src.span(call_member(node).unwrap_or(node)), true,
            format!("Shorten {vector} to at most {length} elements. The vector keeps its allocated capacity. The call returns () (unit)."), vec![
                branch("Requested length is smaller", format!("Keep the first {length} elements in their current order and drop the rest."), "()"),
                branch("Requested length is at least the current length", "Keep all the elements without growing the vector.", "()"),
            ], None);
    }

    fn vec_drain(&mut self, node: Node<'_>) {
        let Some((vector, receiver_argument)) = receiver(node) else { return };
        let Some(range) = argument(node, usize::from(receiver_argument.is_some())) else { return };
        let vector = self.written(vector, 80);
        let whole = range.kind() == "range_expression" && self.src.text(range) == "..";
        let mut text = if whole {
            format!("Remove every element from {vector}.")
        } else if range.kind() == "range_expression" && self.src.text(range).starts_with("..")
            && let Some(end) = named_children(range).into_iter().find(|bound| !bound.is_extra())
            && children(range).iter().any(|token| token.kind() == "..")
        {
            format!("Remove elements from the start of {vector} up to, but not including, index {}.", self.written(end, 80))
        } else {
            format!("Remove the elements in {} from {vector}.", self.written(range, 80))
        };
        text.push_str(" The Drain iterator holds a mutable borrow of the vector and yields removed elements by value. Dropping it also drops any removed elements that it has not yielded.");
        if expression_result_is_discarded(node) {
            text.push_str(" This statement drops the iterator without using it, so it discards all selected elements.");
        }
        if !whole {
            text.push_str(" An invalid index range, such as one extending past the vector's length, causes a panic.");
        }
        self.at(node, true, detail("Remove elements with Vec::drain", text));
    }

    fn inclusive_range_contains(&mut self, node: Node<'_>) {
        let Some((range, receiver_argument)) = receiver(node) else { return };
        let Some(item) = argument(node, usize::from(receiver_argument.is_some())) else { return };
        let item = if item.kind() == "reference_expression"
            && !children(item).iter().any(|token| token.kind() == "raw")
        {
            item.child_by_field_name("value").unwrap_or(item)
        } else {
            item
        };
        let range = unparen(range);
        let item = self.written(item, 80);
        let tokens = children(range);
        let bounds = tokens.iter().position(|token| token.kind() == "..=").and_then(|position| {
            if range.kind() != "range_expression" || range.has_error() { return None; }
            let operand = |token: &&Node<'_>| token.is_named() && !token.is_extra();
            let start = tokens[..position].iter().find(operand)?;
            let end = tokens[position + 1..].iter().find(operand)?;
            Some((self.written(*start, 60), self.written(*end, 60)))
        });
        let text = match bounds {
            Some((start, end)) => format!("Return true if {item} is at least {start} and at most {end}. Both endpoints are included. If either comparison fails, including incomparable values, return false."),
            None => format!("Return true if the range has not been exhausted and {item} is at or above its current start and at or below its current end. Both endpoints are included. If either comparison fails, including incomparable values, return false."),
        };
        self.at(node, true, detail("Check both inclusive bounds", text));
    }

    fn utf8(&mut self, node: Node<'_>, conversion: &str) {
        let Some(callee) = node.child_by_field_name("function") else { return };
        let member = callee.child_by_field_name("name").unwrap_or(callee);
        let (input, valid, invalid) = match conversion {
            "borrowed" => (
                "Check whether the borrowed bytes form UTF-8 text.",
                branch("Valid UTF-8", "View the same bytes as text through a shared reference.", "Ok(&str)"),
                branch("Invalid UTF-8", "Report where UTF-8 validation stopped. The original bytes remain unchanged.", "Err(Utf8Error)"),
            ),
            "owned" => (
                "Take ownership of the byte vector and check whether it forms UTF-8 text.",
                branch("Valid UTF-8", "Use the vector's buffer as an owned String without copying the bytes.", "Ok(String)"),
                branch("Invalid UTF-8", "Keep the original vector in the error. Recover it with into_bytes() on the error.", "Err(FromUtf8Error)"),
            ),
            "lossy" => (
                "Decode the borrowed bytes as UTF-8. The returned Cow can hold borrowed or owned text.",
                branch("Valid UTF-8", "Borrow the input as text without allocating a new string.", "Cow::Borrowed(&str)"),
                branch("Invalid UTF-8", "Create an owned string and replace invalid UTF-8 sequences with � (U+FFFD).", "Cow::Owned(String)"),
            ),
            _ => return,
        };
        self.paths(node, self.src.span(member), true, input.into(), vec![valid, invalid], self.known_type(node));
    }

    fn string_insert(&mut self, node: Node<'_>) {
        let Some((text, receiver_argument)) = receiver(node) else { return };
        let index = usize::from(receiver_argument.is_some());
        let (Some(position), Some(character)) = (argument(node, index), argument(node, index + 1)) else { return };
        let text = self.written(text, 80);
        let character = self.written(character, 80);
        let (label, action) = if position.kind() == "integer_literal" && self.src.text(position) == "0" {
            ("Insert a character at the start", format!("Insert {character} at the start of {text}, changing the existing String. Byte offset 0 is the start of any String, including an empty one."))
        } else {
            let position = self.written(position, 80);
            ("Insert a character at a byte offset", format!("Insert {character} at byte offset {position} in {text}, changing the existing String. An offset equal to the byte length appends the character. The call panics if the offset is greater than the byte length or falls inside a UTF-8 character."))
        };
        self.at(node, true, detail(label, action));
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

    fn once_lock_call(&mut self, node: Node<'_>, name: &str) {
        let Some((receiver, _)) = receiver(node) else { return };
        let member = call_member(node).unwrap_or(node);
        let receiver = self.short(receiver);
        let (input, branches) = match name {
            "take" => {
                let mut expr = node;
                while let Some(parent) = expr.parent().filter(|parent| parent.kind() == "parenthesized_expression") {
                    expr = parent;
                }
                let discarded = expr.parent().is_some_and(|parent| parent.kind() == "expression_statement"
                    && children(parent).iter().any(|token| token.kind() == ";"));
                let output = |value: &str| {
                    if discarded { format!("Discarded {value}") } else { value.into() }
                };
                (format!("Take the contents of the cell {receiver}"), vec![
                    branch("Initialized", "Move the stored value out and leave the cell uninitialized.", output("Some(previous value)")),
                    branch("Uninitialized", "Leave the cell uninitialized.", output("None")),
                ])
            }
            "into_inner" => (format!("Consume the cell {receiver} and take any stored value."), vec![
                branch("Initialized", "Move the stored value out of the cell.", "Some(value)"),
                branch("Uninitialized", "The cell contains no value to move out.", "None"),
            ]),
            "get_mut" => (format!("Try to borrow the contents of {receiver} for writing"), vec![
                branch("Initialized", "Borrow the stored value mutably so the caller can change it.", "Some(mutable reference)"),
                branch("Uninitialized", "Leave the cell empty.", "None"),
            ]),
            "get_or_init" => (format!("Read or initialize {receiver}"), vec![
                branch("Initialized", "Reuse the stored value and skip this initializer.", "Shared reference"),
                branch("This initializer returns", "Run this initializer and store its result.", "Shared reference to the stored result"),
                branch("Another thread finishes initialization first", "Use its stored value, waiting if needed.", "Shared reference to the stored result"),
                branch("This initializer panics", "Propagate the panic. This initialization attempt leaves the cell uninitialized.", "No value is returned"),
            ]),
            _ => return,
        };
        self.paths(node, self.src.span(member), true, input, branches, None);
    }

    fn future_preparation(&mut self, node: Node<'_>, family: Family) {
        let Some(callee) = node.child_by_field_name("function") else { return };
        let callee = if callee.kind() == "generic_function" {
            callee.child_by_field_name("function").unwrap_or(callee)
        } else {
            callee
        };
        let member = method_call(node).map(|(_, member)| member)
            .or_else(|| callee.child_by_field_name("name")).unwrap_or(callee);
        let (label, steps) = match family {
            Family::FutureReady => {
                let Some(value) = argument(node, 0) else { return };
                ("Make a future from a value", vec![
                    self.step(value, "Before the call", "Evaluate the argument now, including any effects from evaluating it."),
                    self.step(node, "On the first poll", "The future immediately returns the stored value as Poll::Ready(value)."),
                ])
            }
            Family::BoxPin => {
                let Some(value) = argument(node, 0) else { return };
                let future = value.kind() == "async_block" || self.op(value).is_some_and(|op| op.family == Family::FutureReady);
                let (label, text) = if future {
                    ("Put the future in a pinned box", "Box the future and return Pin<Box<T>>. Pinning keeps a future that relies on its address in place while the box handle can move. Polling the future is a separate operation.")
                } else {
                    ("Put the value in a pinned box", "Box the value and return Pin<Box<T>>. For values that rely on pinning, the box keeps the value at a stable address while the box handle can move.")
                };
                (label, vec![
                    self.step(value, "Before the call", "Evaluate the argument to get the value to box."),
                    self.step(node, "During this call", text),
                ])
            }
            _ => return,
        };
        self.push(self.src.span(node), self.src.span(member), true, ReadingContent::Steps { label: label.into(), steps });
    }

    fn ordered_push(&mut self, node: Node<'_>) {
        let Some((member, future)) = ordered_push_parts(node) else { return };
        self.ordered_push_steps(self.src.span(node), self.src.span(member), self.src.span(future), None);
    }

    fn ordered_push_steps(&mut self, anchor: Span, member: Span, future: Span, poll: Option<Span>) {
        let linked = poll.is_some();
        let steps = vec![
            ReadingStep {
                span: future,
                timing: "Before insertion".into(),
                action: "Evaluate the argument to obtain the future. Any work done by that expression happens here.".into(),
            },
            ReadingStep {
                span: member.clone(),
                timing: "During push_back".into(),
                action: "Put the future at the back of the queue without polling it.".into(),
            },
            ReadingStep {
                span: poll.unwrap_or_else(|| member.clone()),
                timing: "When the queue is polled".into(),
                action: format!("{}Polling the queue lets futures make progress concurrently. They can finish in a different order. The queue yields outputs in queue order, waiting for earlier entries when needed.",
                    if linked { "This select can poll the queue in this binding through the highlighted next() when its branch is enabled and polled. The insertion handler runs after a branch has already been chosen. " } else { "" }),
            },
        ];
        self.push(anchor, member, true, ReadingContent::Steps { label: "Queue a future at the back".into(), steps });
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

    fn ordering_call(&mut self, node: Node<'_>, name: &str) {
        let Some(member) = call_member(node) else { return };
        if name == "total_cmp" {
            self.push(self.src.span(node), self.src.span(member), true, detail(
                "Compare floating-point values in a total order",
                "Return Less, Equal, or Greater. This comparison also places NaN values in the order and treats negative zero as less than positive zero.",
            ));
            return;
        }
        let Some((first, receiver_argument)) = receiver(node) else { return };
        let Some(tie) = argument(node, usize::from(receiver_argument.is_some())) else { return };
        let first = self.written(first, 100);
        let tie = self.written(tie, 100);
        let (input, use_tie, keep_first) = if name == "then_with" {
            (
                format!("Use {first} as the first comparison. Evaluate {tie} to obtain the tie-breaking function."),
                "Call the tie-breaking function and use its comparison result.",
                "Keep the first result and skip calling the tie-breaking function.",
            )
        } else {
            (
                format!("Evaluate {first} first. If it completes, evaluate the tie-breaker {tie} before this call even when the first comparison already decides the order."),
                "Use the tie-breaking result that has already been evaluated.",
                "Keep the first result and discard the already evaluated tie-breaking result.",
            )
        };
        self.paths(node, self.src.span(member), true, input, vec![
            branch("First comparison is Equal", use_tie, "Tie-breaking ordering"),
            branch("First comparison is Less or Greater", keep_first, "First ordering"),
        ], self.known_type(node));
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

    fn iterator_fold(&mut self, node: Node<'_>) {
        let Some((_, receiver_argument)) = receiver(node) else { return };
        let first = usize::from(receiver_argument.is_some());
        let (Some(seed), Some(callback)) = (argument(node, first), argument(node, first + 1)) else { return };
        let Some(callee) = node.child_by_field_name("function") else { return };
        let callee = if callee.kind() == "generic_function" {
            let Some(callee) = callee.child_by_field_name("function") else { return };
            callee
        } else {
            callee
        };
        let member = callee.child_by_field_name("field").or_else(|| callee.child_by_field_name("name")).unwrap_or(callee);
        let steps = vec![
            self.step(seed, "Starting value", "Use this argument as the accumulator, the value carried from one item to the next."),
            self.step(callback, "For each item during this call", "Process items in the order this iterator supplies them. Call this function with the current accumulator first and the item second. Its return value becomes the next accumulator."),
            self.step(node, "When iteration finishes", "Return the final accumulator. If there are no items, return the starting value without calling the function."),
        ];
        self.push(self.src.span(node), self.src.span(member), true, ReadingContent::Steps {
            label: "Accumulate a value across iterator items".into(), steps,
        });
    }

    fn iterator_zip(&mut self, node: Node<'_>) {
        let Some((left, receiver_argument)) = receiver(node) else { return };
        let Some(right) = argument(node, usize::from(receiver_argument.is_some())) else { return };
        let left = self.written(left, 80);
        let right = self.written(right, 80);
        self.at(node, true, detail(
            "Pair items from two inputs",
            format!(
                "Create an iterator over pairs from the current positions of both inputs. Each pair puts the item from {left} first and the item from {right} second. When advancing from the front, try the first input first. If either input yields no item for a request, the paired iterator yields None for that request. The first input may already have supplied an item that goes unpaired."
            ),
        ));
    }

    fn sequence(&mut self, node: Node, consumer: &str) {
        let Some((mut source, _)) = receiver(node) else { return };
        let option_list = consumer == "collect" && self.collects_option_list(node, unparen(source));
        let result_vec = consumer == "collect" && self.collects_result_vec(node, unparen(source));
        let mut steps = Vec::new();
        while let Some(operation) = self.op(unparen(source)).filter(|op| matches!(op.family, Family::Iter | Family::ParIter)) {
            let action = match operation.name.as_str() {
                "iter" => "Read each item through a shared reference",
                "iter_mut" => "Read each item through a reference that permits changes",
                "into_iter" => "Take the iterator source by value",
                "par_iter" => "Read items by reference, allowing parallel processing",
                "par_iter_mut" => "Read disjoint items by mutable reference, allowing parallel processing",
                "into_par_iter" => "Take the source by value for parallel processing",
                "zip" if operation.family == Family::Iter => "Yield the next pair in input order if both inputs supply an item. Otherwise, yield None for this request",
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
        if steps.is_empty() && !result_vec {
            self.at(node, true, detail("Consume the iterator", "Drive the iterator to produce a result."));
            return;
        }
        steps.reverse();
        let final_step = if option_list {
            self.step(node, "When collecting", "If an item is None, stop asking for items and return None. If the iterator reaches the end, return Some(list) containing the unwrapped values. The list can be empty.")
        } else if result_vec {
            self.step(node, "When collecting", "Read the results in iterator order and put each Ok value into a vector. Stop at the first Err, discard the partial vector, and return Err(error) without asking for more items. Effects that already happened remain. If the iterator reaches the end without an error, return Ok with the vector, which can be empty.")
        } else {
            self.step(node, "Now", format!("Drive the sequence with {consumer}."))
        };
        let label = if steps.is_empty() { "Collect successes or return the first error" } else { "Follow an item through the iterator" };
        steps.push(final_step);
        self.steps(node, label, steps);
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

    fn collects_result_vec(&self, collect: Node<'_>, mapped: Node<'_>) -> bool {
        if !self.op(collect).is_some_and(|op| op.family == Family::Iter && op.name == "collect") {
            return false;
        }
        if !self.maps_to_standard_result(mapped) {
            return false;
        }
        let Some(callee) = collect.child_by_field_name("function") else { return false };
        let target = if callee.kind() == "generic_function" {
            single_type_argument(callee)
        } else {
            let mut value = collect;
            while let Some(parent) = value.parent().filter(|parent| parent.kind() == "parenthesized_expression") {
                value = parent;
            }
            value.parent().filter(|parent| {
                parent.kind() == "let_declaration" && parent.child_by_field_name("value") == Some(value)
            }).and_then(|declaration| declaration.child_by_field_name("type"))
        };
        let Some(identities) = self.evidence.default_types else { return false };
        matches!(target.and_then(|ty| written_default_collection(ty, identities)), Some(DefaultCollection {
            name: "Vec", container: Some(DefaultType::Result), mutable_reference: false,
        }))
    }

    fn maps_to_standard_result(&self, mapped: Node<'_>) -> bool {
        if !self.op(mapped).is_some_and(|op| op.family == Family::Iter && op.name == "map") {
            return false;
        }
        let Some((_, position)) = receiver(mapped) else { return false };
        let Some(closure) = argument(mapped, usize::from(position.is_some())) else { return false };
        if closure.kind() != "closure_expression" || children(closure).iter().any(|token| token.kind() == "async") {
            return false;
        }
        if let Some(ty) = closure.child_by_field_name("return_type") {
            return self.evidence.default_types.is_some_and(|types| written_result_type(ty, types));
        }
        let Some(value) = closure_result(closure).map(unparen) else { return false };
        self.evidence.result_calls.contains(&range(value)) || self.op(value).is_some_and(|op| {
            matches!((op.family, op.name.as_str()),
                (Family::Result, "Ok") | (Family::Option, "ok_or" | "ok_or_else"))
        })
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
            (Family::Option, "zip") => {
                let Some(other) = argument(node, callback_arg) else { return };
                format!("Pair the values from {} and {} when both options contain a value.",
                    self.written(recv, 120), self.written(other, 120))
            }
            (Family::Option, "replace") => format!("Replace the contents of {}", self.short(recv)),
            (Family::Option, "get_or_insert") => format!("Keep or fill {}", self.short(recv)),
            (Family::Option, "take_if") => format!("Take the contents of {} if the test passes", self.short(recv)),
            (Family::Option, "or") => "Choose which option to return".into(),
            (Family::Option, "and") => "Use the first option to decide whether to return the second".into(),
            (Family::Option, "as_deref_mut") => format!("Borrow through {}. The original option stays in place and can be used again when the borrow ends.", self.short(recv)),
            (Family::Option, "map_or" | "map_or_else") => format!("Transform {} or use a fallback", self.short(recv)),
            (Family::Option | Family::Result, "unwrap_or_default") => format!("Read {} or use its default", self.short(recv)),
            _ => self.short(recv),
        };
        let mut span = self.src.span(node);
        if matches!((family, name), (Family::Option, "replace" | "get_or_insert" | "take_if" | "or" | "and" | "as_deref_mut" | "map_or" | "map_or_else" | "unwrap_or_default") | (Family::Result, "unwrap_or_default")) {
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
        if family == Family::Option && matches!(name, "or" | "and") && let Some(other) = argument(node, callback_arg) {
            let first = if name == "or" { "Some" } else { "None" };
            self.steps(node, "Evaluate both options in order", vec![
                self.step(recv, "First", "Evaluate the first option. If evaluation leaves this scope, the second option is skipped."),
                self.step(other, "If the first evaluation completes", format!("Evaluate the second option even if the first is {first}. This evaluation can leave the scope before the method runs.")),
            ]);
        }
        if family == Family::Option && matches!(name, "map_or" | "map_or_else")
            && let (Some(fallback), Some(mapper)) = (argument(node, callback_arg), argument(node, callback_arg + 1))
        {
            let fallback_label = if name == "map_or" { "fallback value" } else { "expression that supplies the fallback function" };
            self.steps(node, "Evaluate the arguments before checking the option", vec![
                self.step(fallback, "If the option expression completes", format!("Evaluate the {fallback_label}, even if the option contains a value.")),
                self.step(mapper, "If the fallback argument completes", "Evaluate the expression that supplies the transformation function, even if the option is empty."),
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
        let is_matches = resolved.is_some_and(|op| op.name == "matches");
        if is_select {
            self.select_branches(node);
        }
        if is_matches {
            self.matches_propagation(node);
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
            Some("join") => (
                "Wait for every future",
                "Poll the futures concurrently within the current task. Once all of them finish, return their outputs as a tuple in the order the futures are written. If a future returns Err, keep that error in its tuple position and keep waiting for the others.",
            ),
            Some("try_join") => (
                "Wait for every future to succeed",
                "Poll the futures concurrently within the current task. If all return Ok, return Ok with a tuple of their values in the order the futures are written. When the macro encounters an Err, return it and drop the futures still in progress. Work already started elsewhere can continue, and effects already performed may still be observable.",
            ),
            _ => (
                "Macro invocation",
                "Rust expands this macro during compilation using the tokens written at the call.",
            ),
        };
        let definition = resolved.and_then(|op| op.definition.clone());
        let mut text = text.to_owned();
        if resolved.is_some_and(|op| matches!(op.name.as_str(), "join" | "try_join"))
            && let Some(tree) = named_children(node).into_iter().find(|child| child.kind() == "token_tree" && !child.has_error())
        {
            let tokens: Vec<_> = children(tree).into_iter().filter(|token| !token.is_extra()).collect();
            if let [_, bias, separator, ..] = tokens.as_slice()
                && self.src.text(*bias) == "biased" && separator.kind() == ";"
            {
                text.push_str(" With `biased;`, poll in written order. Otherwise, Tokio rotates which future it checks first.");
            }
        }
        self.push(
            self.src.span(node),
            self.src.span(name),
            resolved.is_some(),
            ReadingContent::Detail {
                label: label.into(),
                text,
                definition,
                borrow: false,
            },
        );
    }

    fn matches_propagation(&mut self, node: Node<'_>) {
        let Some(tokens) = named_children(node).into_iter().find(|child| child.kind() == "token_tree") else { return };
        if tokens.has_error() || !self.src.text(tokens).contains('?') {
            return;
        }
        let Some(start) = tokens.child(0).map(|open| open.end_byte()) else { return };
        let Some(parsed) = children(tokens).into_iter().filter(|token| token.kind() == ",")
            .find_map(|comma| parse_macro_operand(&self.src.text[start..comma.start_byte()]))
        else { return };
        let Some(mut value) = macro_operand(&parsed) else { return };
        while value.kind() == "parenthesized_expression" {
            let Some(inner) = value.named_children(&mut value.walk()).find(|child| !child.is_extra()) else { return };
            value = inner;
        }
        let exit = match boundary(node) {
            Some(scope) if scope.kind() == "async_block" => "Finish this async block early".into(),
            Some(scope) if scope.kind() == "try_block" => "Finish this try block early".into(),
            Some(scope) if scope.kind() == "closure_expression" => "Return early from this closure".into(),
            Some(scope) => match scope.child_by_field_name("name") {
                Some(name) => format!("Return early from {}", self.src.text(name)),
                None => "Return early from this function".into(),
            },
            None => "Leave this scope early".into(),
        };
        let mut pending = vec![value];
        while let Some(part) = pending.pop() {
            if matches!(part.kind(), "closure_expression" | "async_block" | "try_block" | "gen_block" | "const_block" | "macro_invocation")
                || part.kind().ends_with("_item")
            {
                continue;
            }
            if part.kind() == "try_expression"
                && let Some(token) = children(part).into_iter().find(|token| token.kind() == "?" && !token.is_missing())
            {
                let start = start + token.start_byte() - MACRO_OPERAND_PREFIX.len();
                let end = start + token.byte_range().len();
                if self.src.text.get(start..end) != Some("?") {
                    continue;
                }
                let span = self.src.index.span(self.src.file, start, end);
                let kind = self.evidence.operations.get(&(start, end)).and_then(Option::as_ref).map(|op| op.family);
                let (keep, take, stop, propagate) = match kind {
                    Some(Family::Result) => ("Success", "Take the value inside Ok", "Error", format!("{exit} with the error before testing the pattern")),
                    Some(Family::Option) => ("Present", "Take the value inside Some", "Missing", format!("{exit} with None before testing the pattern")),
                    _ => ("Continue", "Take the value produced by ?", "Propagate", format!("{exit} before testing the pattern")),
                };
                let next = if part == value { "Test the pattern with this value" } else { "Continue with the value" };
                self.paths(node, span, true, "Evaluate the first argument of matches!".into(), vec![
                    branch(keep, take, next),
                    branch(stop, propagate, "This pattern test produces no bool"),
                ], None);
            }
            pending.extend(named_children(part).into_iter().rev());
        }
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
        let registry_select = self.evidence.registry_selects.contains(&range(node));
        let mut queue_calls = HashSet::new();
        let mut wait_calls = Vec::new();
        for (&range, operation) in &self.evidence.operations {
            if tree.start_byte() <= range.0 && range.1 <= tree.end_byte() {
                match operation.as_ref().map(|op| op.family) {
                    Some(Family::FuturesOrderedPush) => { queue_calls.insert(range); }
                    Some(Family::BroadcastRecv | Family::StreamNext) if registry_select => wait_calls.push(range),
                    _ => {}
                }
            }
        }
        let link_poll = !queue_calls.is_empty() && registry_select;
        let mut start = if let [_, bias, separator, ..] = tokens.as_slice()
            && self.src.text(*bias) == "biased" && separator.kind() == ";"
        {
            3
        } else {
            1
        };
        let mut steps = Vec::new();
        let mut handlers = Vec::new();
        let mut polls = HashMap::new();
        let mut broadcasts = Vec::new();
        let mut handlers_complete = true;
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
                if wait_calls.iter().any(|&(start, end)| first.start_byte() <= start && end <= last.end_byte())
                    && let Some((start, parsed)) = self.parse_select_wait(&before[equal + 1..])
                    && let Some(call) = macro_operand(&parsed).map(unparen).filter(|call| call.kind() == "call_expression")
                    && let Some(member) = call_member(call)
                    && let Some(proof) = self.evidence.operations.get(&macro_operand_range(start, member)).and_then(Option::as_ref)
                {
                    let (first, last) = macro_operand_range(start, call);
                    let span = self.src.index.span(self.src.file, first, last);
                    if link_poll && proof.family == Family::StreamNext
                        && let Some((receiver, _)) = select_queue_call(call)
                        && let Some(binding) = self.ordered_queue_binding(macro_operand_range(start, receiver))
                    {
                        polls.entry(binding).and_modify(|previous| *previous = None).or_insert(Some(span));
                    } else if proof.family == Family::BroadcastRecv {
                        broadcasts.push(span);
                    }
                }
                let (at, action) = exit.unwrap_or((*token, "run this branch"));
                steps.push(self.step(at, "When ready and the pattern matches", format!("{wait} → {action}")));
            } else if let [otherwise] = before && self.src.text(*otherwise) == "else" {
                let (at, action) = exit.unwrap_or((*token, "Run the else branch"));
                steps.push(self.step(at, "If all branches are disabled", action));
            } else {
                break;
            }
            let (handler_end, consumed) = if body.first().is_some_and(|token| {
                token.kind() == "token_tree" && self.src.text(*token).starts_with('{')
            }) {
                // A bare block handler can omit its comma. The next prefix must still
                // look like a branch before we attribute a handler to it.
                (body[0].end_byte(), 1 + usize::from(body.get(1).is_some_and(|token| token.kind() == ",")))
            } else if let Some(comma) = select_handler_comma(self.src, body) {
                (body[comma].start_byte(), comma + 1)
            } else {
                let end = tree.end_byte().saturating_sub(1);
                if !wait_calls.is_empty() {
                    handlers_complete &= body.first().and_then(|first| self.src.text.get(first.start_byte()..end))
                        .and_then(parse_macro_operand).is_some();
                }
                (end, body.len())
            };
            if let Some(first) = body.first()
                && queue_calls.iter().any(|&(at, stop)| first.start_byte() <= at && stop <= handler_end)
            {
                handlers.push((first.start_byte(), handler_end));
            }
            start = arrow + 1 + consumed;
            if start >= tokens.len() {
                break;
            }
        }
        if !steps.is_empty() {
            self.steps(node, "Wait for one eligible branch", steps);
        }
        if handlers_complete && start >= tokens.len().saturating_sub(1) {
            for call in broadcasts {
                let written = truncate(&self.src.text[call.start_byte as usize..call.end_byte as usize], 80);
                self.broadcast_receive_paths(node, call, written, true);
            }
        } else {
            polls.clear();
        }
        for (start, end) in handlers {
            self.select_handler_queue_calls(node, start, end, &queue_calls, &polls);
        }
    }

    fn parse_select_wait(&self, tokens: &[Node<'_>]) -> Option<(usize, tree_sitter::Tree)> {
        let start = tokens.first()?.start_byte();
        let end = match tokens.windows(2).position(|pair| pair[0].kind() == "," && self.src.text(pair[1]) == "if") {
            Some(index) => {
                let guard = self.src.text.get(tokens[index + 1].end_byte()..tokens.last()?.end_byte())?;
                parse_macro_operand(guard)?;
                tokens[index].start_byte()
            }
            None => tokens.last()?.end_byte(),
        };
        Some((start, parse_macro_operand(self.src.text.get(start..end)?)?))
    }

    fn ordered_queue_binding(&self, site: Range) -> Option<Range> {
        let first = self.evidence.uses.partition_point(|&(start, _, _)| start < site.0);
        let mut usages = self.evidence.uses[first..].iter().take_while(|&&(start, _, _)| start == site.0)
            .filter(|&&(_, end, _)| end == site.1);
        let &(_, _, symbol) = usages.next()?;
        if usages.any(|&(_, _, other)| symbol != other) {
            return None;
        }
        let binding = *self.evidence.definitions?.get(symbol)?;
        self.evidence.ordered_queue_bindings.contains(&binding).then_some(binding)
    }

    fn select_handler_queue_calls(&mut self, select: Node<'_>, start: usize, end: usize, queue_calls: &HashSet<Range>, polls: &HashMap<Range, Option<Span>>) {
        let Some(parsed) = self.src.text.get(start..end).and_then(parse_macro_operand) else { return };
        let Some(handler) = macro_operand(&parsed) else { return };
        let original = |node| macro_operand_range(start, node);
        let mut pending = vec![handler];
        while let Some(part) = pending.pop() {
            if matches!(part.kind(), "closure_expression" | "async_block" | "gen_block" | "const_block" | "macro_invocation")
                || part.kind().ends_with("_item")
            {
                continue;
            }
            if part.kind() == "call_expression" && let Some((member, future)) = ordered_push_parts(part) {
                let (first, last) = original(member);
                if queue_calls.contains(&(first, last)) {
                    let (arg_start, arg_end) = original(future);
                    let poll = select_queue_call(part).and_then(|(receiver, _)| self.ordered_queue_binding(original(receiver)))
                        .and_then(|binding| polls.get(&binding)).cloned().flatten();
                    self.ordered_push_steps(self.src.span(select), self.src.index.span(self.src.file, first, last),
                        self.src.index.span(self.src.file, arg_start, arg_end), poll);
                }
            }
            pending.extend(named_children(part).into_iter().rev());
        }
    }

    fn closure(&mut self, node: Node<'_>) {
        let Some(parameters) = node.child_by_field_name("parameters") else {
            return;
        };
        let tokens = children(node);
        let moves = tokens.iter().any(|token| token.kind() == "move");
        let async_ = tokens.iter().any(|token| token.kind() == "async");
        if let Some(predicate) = self.predicate_callback(node) && let Some(body) = node.child_by_field_name("body") {
            let body = unparen(body);
            let tail = if body.kind() == "block" {
                block_statements(body).next_back().and_then(statement_tail)
            } else {
                Some(body)
            };
            if let Some(tail) = tail {
                self.predicate_decision(unparen(tail), tail, predicate);
            }
        }
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
        self.map_callback_entry(node, parameters, async_);
        if !async_ {
            self.fold_callback_entry(node, parameters);
            self.ordering_callback_entry(node, parameters);
            self.sort_callback_entry(node, parameters);
        }
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

    fn map_callback_entry(&mut self, closure: Node<'_>, parameters: Node<'_>, async_: bool) {
        let Some(call) = direct_argument_call(closure) else { return };
        let Some((_, receiver_argument)) = receiver(call) else { return };
        if argument(call, usize::from(receiver_argument.is_some())) != Some(closure) {
            return;
        }
        let Some(operation) = self.op(call).filter(|operation| operation.name == "map") else { return };
        let (label, text) = match (operation.family, async_) {
            (Family::Iter, false) => (
                "Transform an item from the iterator",
                "Iterator::map creates an iterator and saves this function for later. When a consumer reads a mapped item, the iterator passes the input item to this function and yields the returned value.",
            ),
            (Family::Option, false) => (
                "Transform the value inside Some",
                "During Option::map, Some passes its value to this function once and wraps the returned value in a new Some. None skips this function and stays None.",
            ),
            (Family::Iter, true) => (
                "Create a future for an iterator item",
                "Iterator::map saves this async function for later. When a consumer reads a mapped item, the iterator passes the input item to this function and yields the future it creates. This body runs when the future is polled.",
            ),
            (Family::Option, true) => (
                "Create a future for the value inside Some",
                "During Option::map, Some passes its value to this async function once and wraps the future it creates in a new Some. This body runs when the future is polled. None skips this function and stays None.",
            ),
            _ => return,
        };
        self.readings.push(ReadingFact {
            anchor: self.src.header_span(closure, closure.child_by_field_name("body")),
            span: self.src.span(parameters),
            basis: ValueBasis::TypeAnalysis,
            content: detail(label, text),
            expression_target: Some(Concept::Entry),
        });
    }

    fn sort_callback_entry(&mut self, closure: Node<'_>, parameters: Node<'_>) {
        let Some(call) = direct_argument_call(closure) else { return };
        if !self.op(call).is_some_and(|op| op.family == Family::Slice && matches!(op.name.as_str(), "sort_by" | "sort_unstable_by")) {
            return;
        }
        let Some((_, receiver_argument)) = receiver(call) else { return };
        if argument(call, usize::from(receiver_argument.is_some())) != Some(closure) {
            return;
        }
        self.readings.push(ReadingFact {
            anchor: self.src.header_span(closure, closure.child_by_field_name("body")),
            span: self.src.span(parameters),
            basis: ValueBasis::TypeAnalysis,
            content: detail("Compare two elements during sorting", "The sort supplies shared references to two elements. Return Less to place the first before the second, Greater to place it after the second, or Equal to tie them."),
            expression_target: Some(Concept::Entry),
        });
    }

    fn ordering_callback_entry(&mut self, closure: Node<'_>, parameters: Node<'_>) {
        let Some(call) = direct_argument_call(closure) else { return };
        if !self.op(call).is_some_and(|op| op.family == Family::Ordering && op.name == "then_with") {
            return;
        }
        let Some((_, receiver_argument)) = receiver(call) else { return };
        if argument(call, usize::from(receiver_argument.is_some())) != Some(closure) {
            return;
        }
        self.readings.push(ReadingFact {
            anchor: self.src.header_span(closure, closure.child_by_field_name("body")),
            span: self.src.span(parameters),
            basis: ValueBasis::TypeAnalysis,
            content: detail("Break a tie between equal values", "This function runs only when the first comparison returns Equal. Its return value becomes the ordering returned by then_with."),
            expression_target: Some(Concept::Entry),
        });
    }

    fn fold_callback_entry(&mut self, closure: Node<'_>, parameters: Node<'_>) {
        let Some(call) = direct_argument_call(closure) else { return };
        if !matches!(self.op(call).and_then(IteratorCallback::operation), Some(IteratorCallback::Fold))
            || !IteratorCallback::Fold.is_callback(call, closure)
        {
            return;
        }
        self.readings.push(ReadingFact {
            anchor: self.src.header_span(closure, closure.child_by_field_name("body")),
            span: self.src.span(parameters),
            basis: ValueBasis::TypeAnalysis,
            content: detail("Update the accumulator for one item", "During fold, this function receives the accumulator as its first parameter and the current item as its second parameter. Its return value is used for the next item or returned by fold if this is the last item."),
            expression_target: Some(Concept::Entry),
        });
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
                && !crate::syntax::discarded_path(usage)
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
        for (site, content) in crate::raw_mut_pointer_explanation::local(node) { self.push(self.src.span(node), self.src.span(site), false, content); }
        for (site, content) in crate::raw_const_pointer_explanation::local(node) { self.push(self.src.span(node), self.src.span(site), false, content); }
        for (site, content) in crate::shared_slice_reference_explanation::local(self.src, node) { self.push(self.src.span(node), self.src.span(site), false, content); }
        for (site, content) in crate::mutable_reference_type_explanation::local(node) {
            let span = site.parent().and_then(|parent| children(parent).into_iter().find(|part| part.kind() == "&")).map_or_else(|| self.src.span(site), |part| self.src.span(part));
            self.push(self.src.span(node), span, false, content);
        }
        for (site, content) in crate::mutable_slice_reference_explanation::local(self.src, node) { self.push(self.src.span(node), self.src.span(site), false, content); }
        for (site, content) in crate::slice_type_explanation::local(self.src, node) { self.push(self.src.span(node), self.src.span(site), false, content); }
        for (site, content) in crate::shared_reference_type_explanation::local(self.src, node) { self.push(self.src.span(node), self.src.span(site), false, content); }
        for (highlight, content) in crate::array_type_explanation::local(self.src, node) { self.push(self.src.span(node), self.src.span(highlight), false, content); }
        for (ty, content) in crate::tuple_type_explanation::local(self.src, node) {
            let span = if node.child_by_field_name("type") == Some(ty) { children(node).into_iter().find(|part| part.kind() == ":").map_or_else(|| self.src.span(ty), |part| self.src.span(part)) } else { self.src.span(ty) };
            self.push(self.src.span(node), span, false, content);
        }
        for (span, content) in crate::integer_128_explanation::local(self.src, node) { self.push(self.src.span(node), span, false, content); }
        for (site, content) in crate::str_reference_explanation::local(self.src, node) {
            let span = if node.child_by_field_name("type") == Some(site) { children(node).into_iter().find(|part| part.kind() == ":").map_or_else(|| self.src.span(site), |part| self.src.span(part)) } else { self.src.span(site) };
            self.push(self.src.span(node), span, false, content);
        }
        if let Some((ty, content)) = crate::string_type_explanation::local(self.src, node) {
            let span = if node.child_by_field_name("type") == Some(ty) { children(node).into_iter().find(|part| part.kind() == ":").map_or_else(|| self.src.span(ty), |part| self.src.span(part)) } else { self.src.span(ty) };
            self.push(self.src.span(node), span, false, content);
        }
        for (ty, text) in crate::signed_integer_explanation::local_types(self.src, node) {
            let span = if node.child_by_field_name("type") == Some(ty) { children(node).into_iter().find(|part| part.kind() == ":").map_or_else(|| self.src.span(ty), |part| self.src.span(part)) } else { self.src.span(ty) };
            self.push(self.src.span(node), span, false, detail("Signed integer type", text));
        }
        for (ty, content) in crate::unsigned_integer_type_explanation::local(self.src, node) {
            let span = if node.child_by_field_name("type") == Some(ty) { children(node).into_iter().find(|part| part.kind() == ":").map_or_else(|| self.src.span(ty), |part| self.src.span(part)) } else { self.src.span(ty) };
            self.push(self.src.span(node), span, false, content);
        }
        if let Some(content) = crate::shadow_reading::explain(self.src, node) {
            self.at(node, true, content);
        }
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
        if value.is_some() && let Some(wildcard) = crate::syntax::let_wildcard(node) {
            self.push(self.src.span(node), self.src.span(wildcard), false, detail(
                "Ignore the result with let _",
                "Evaluate the expression on the right, including any effects. The wildcard creates no variable. If the expression refers to an existing variable or field, its value stays in place. A temporary result is discarded and cleaned up when its temporary scope ends.",
            ));
            return;
        }
        self.explicit_binding_type(node, pattern);
        if node.kind() == "let_declaration" && value.is_none() && pattern.kind() == "identifier" {
            let name = self.src.norm(pattern);
            let type_description = node.child_by_field_name("type")
                .map(|ty| format!(" with the declared type {}", self.src.norm(ty)))
                .unwrap_or_default();
            self.push(self.src.span(node), self.src.span(pattern), false, detail(
                "Declare a variable without a value",
                format!("Declare {name}{type_description}, but do not assign a value yet. Rust requires it to be initialized on every reachable path before its value is read."),
            ));
        }
        if pattern.kind() == "identifier" {
            self.explain_immutable_binding(node, pattern, value);
            if node.kind() == "let_declaration"
                && value.is_some()
                && let Some(mutability) = children(node).into_iter().find(|child| child.kind() == "mutable_specifier")
            {
                self.push(self.src.span(node), self.src.span(mutability), false, detail(
                    "Create a changeable variable",
                    format!("let creates the variable {} and initializes it with the value on the right. mut allows assigning a new value to this variable later; it does not make other variables changeable.", self.short(pattern)),
                ));
            }
            return;
        }
        let input = value.map_or_else(|| "Input value".into(), |value| self.short(value));
        if let Some(tuple) = crate::reading_tuple_let::tuple_let_pattern(node) {
            self.push(self.src.span(node), self.src.span(tuple), false, detail(crate::reading_tuple_let::TITLE, crate::reading_tuple_let::EXPLANATION));
        }
        if let Some(text) = crate::struct_binding_reading::explanation(node) {
            self.push(self.src.span(node), self.src.span(pattern), false, detail("Destructure named fields", text));
        }
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

    fn explicit_binding_type(&mut self, node: Node<'_>, pattern: Node<'_>) {
        if node.kind() != "let_declaration" || pattern.kind() != "identifier"
            || node.child_by_field_name("value").is_none() {
            return;
        }
        let Some(ty) = node.child_by_field_name("type") else { return };
        let name = self.short(pattern);
        let type_name = self.short(ty);
        self.push(self.src.span(node), self.src.span(ty), false, detail(
            "Explicit variable type",
            format!("The colon introduces the explicitly declared type of {name}: {type_name}. Rust checks the initializer against this type; the annotation describes the variable, not a conversion of its value."),
        ));
    }

    fn let_else(&mut self, node: Node, value: Node, pattern: Node, else_token: Node) {
        let (input, success_text, mismatch_text) = crate::let_else_explanation::explain(&self.short(value), &self.short(pattern));
        let mut success = branch("Pattern matches", success_text, "Continue below");
        success.pin = Some("out".into());
        let mut mismatch = branch("Pattern mismatch", mismatch_text, "Run else");
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
                input,
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

fn expression_result_is_discarded(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent().filter(|parent| parent.kind() == "parenthesized_expression") {
        node = parent;
    }
    node.parent().is_some_and(|parent| {
        parent.kind() == "expression_statement"
            && children(parent).iter().any(|token| token.kind() == ";" && !token.is_missing())
            || crate::syntax::let_wildcard(parent).is_some() && parent.child_by_field_name("value") == Some(node)
    })
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

fn question_mark_consumer(src: Src<'_>, mut node: Node<'_>) -> Option<String> {
    let parent = loop {
        let parent = node.parent()?;
        match parent.kind() {
            "parenthesized_expression" => node = parent,
            "expression_statement" if statement_tail(parent) == Some(node) => node = parent,
            "block" if block_statements(parent).next_back() == Some(node)
                && statement_tail(node).is_some() =>
            {
                node = parent;
            }
            "else_clause" if matches!(node.kind(), "block" | "if_expression") => node = parent,
            "if_expression" if parent.child_by_field_name("consequence") == Some(node)
                || parent.child_by_field_name("alternative") == Some(node) =>
            {
                node = parent;
            }
            "match_arm" if parent.child_by_field_name("value") == Some(node) => {
                let body = parent.parent().filter(|body| body.kind() == "match_block")?;
                let owner = body.parent().filter(|owner| owner.kind() == "match_expression"
                    && owner.child_by_field_name("body") == Some(body))?;
                node = owner;
            }
            _ => break parent,
        }
    };
    let short = |at| truncate(&src.norm(at), 48);
    let generic_call = "Use the value to continue evaluating the call";
    let generic_expression = "Use the value to continue evaluating this expression";
    let continuation = match parent.kind() {
        "arguments" => {
            let call = parent.parent().filter(|call| call.kind() == "call_expression")?;
            let callee = call.child_by_field_name("function")?;
            let callee = if callee.kind() == "generic_function" {
                callee.child_by_field_name("function")?
            } else {
                callee
            };
            let name = match callee.kind() {
                "identifier" | "scoped_identifier" => Some(short(callee)),
                "field_expression" => callee.child_by_field_name("field").map(short),
                _ => None,
            };
            name.map_or_else(|| generic_call.into(), |name| {
                format!("Use the value as an argument to {name}(...)")
            })
        }
        "call_expression" if parent.child_by_field_name("function") == Some(node) =>
            "Use the value as the function to call".into(),
        "field_expression" if parent.child_by_field_name("value") == Some(node) => {
            let mut callee = parent;
            if let Some(generic) = parent.parent().filter(|generic| generic.kind() == "generic_function"
                && generic.child_by_field_name("function") == Some(parent)) {
                callee = generic;
            }
            if let Some((_, method)) = callee.parent().and_then(method_call)
                .filter(|(receiver, _)| unparen(node) == *receiver || node == *receiver)
            {
                format!("Use the value as the receiver of .{}(...) while evaluating the call", short(method))
            } else if let Some(field) = parent.child_by_field_name("field") {
                format!("Read the .{} field from the value", short(field))
            } else {
                "Use the value to continue evaluating the field access".into()
            }
        }
        "binary_expression" => {
            if let (Some(operator), Some(left), Some(right)) = (
                parent.child_by_field_name("operator"), parent.child_by_field_name("left"), parent.child_by_field_name("right"),
            ) && !matches!(src.text(operator), "&&" | "||") && (node == left || node == right) {
                let (side, other) = if node == left { ("left", right) } else { ("right", left) };
                let operator = short(operator);
                if matches!(unparen(other).kind(), "identifier" | "field_expression" | "integer_literal" | "float_literal") {
                    format!("Use the value as the {side} operand of {operator} with {}", short(other))
                } else {
                    format!("Use the value as the {side} operand of {operator}")
                }
            } else {
                generic_expression.into()
            }
        }
        "compound_assignment_expr" if parent.child_by_field_name("right") == Some(node) => {
            if let (Some(left), Some(operator)) = (
                parent.child_by_field_name("left"), parent.child_by_field_name("operator"),
            ) {
                format!("Use the value on the right of {} {}", short(left), short(operator))
            } else {
                generic_expression.into()
            }
        }
        "unary_expression" | "type_cast_expression" | "index_expression"
        | "reference_expression" | "await_expression" | "try_expression" | "tuple_expression"
        | "array_expression" | "field_initializer" | "base_field_initializer"
        | "assignment_expression" | "compound_assignment_expr" | "range_expression" =>
            generic_expression.into(),
        _ => return None,
    };
    Some(continuation)
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

const MACRO_OPERAND_PREFIX: &str = "fn operand() { let _ = ";

fn parse_macro_operand(text: &str) -> Option<tree_sitter::Tree> {
    // The let wrapper accepts exactly one expression and provides a fixed byte offset.
    let wrapped = format!("{MACRO_OPERAND_PREFIX}{text}\n; }}");
    let parsed = crate::parse(&wrapped).filter(|tree| !tree.root_node().has_error())?;
    let value = macro_operand(&parsed)?;
    let body = value.parent()?.parent()?;
    if body.named_children(&mut body.walk()).filter(|child| !child.is_extra()).count() != 1
        || value.start_byte() < MACRO_OPERAND_PREFIX.len() || value.end_byte() > MACRO_OPERAND_PREFIX.len() + text.len()
    {
        return None;
    }
    Some(parsed)
}

fn macro_operand(tree: &tree_sitter::Tree) -> Option<Node<'_>> {
    tree.root_node().named_child(0)?.child_by_field_name("body")?
        .named_child(0)?.child_by_field_name("value")
}

fn macro_operand_range(start: usize, node: Node<'_>) -> Range {
    let first = start + node.start_byte() - MACRO_OPERAND_PREFIX.len();
    (first, first + node.byte_range().len())
}

fn select_queue_call(call: Node<'_>) -> Option<(Node<'_>, Node<'_>)> {
    let (receiver, member) = if let Some((receiver, member)) = method_call(call) {
        (receiver, member)
    } else {
        let member = call.child_by_field_name("function")?.child_by_field_name("name")?;
        let reference = argument(call, 0).filter(|arg| arg.kind() == "reference_expression")?;
        (reference.child_by_field_name("value")?, member)
    };
    let receiver = unparen(receiver);
    (receiver.kind() == "identifier").then_some((receiver, member))
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
        "map_or" => (
            "Pass the value inside Some to the transformation",
            "Transformation result",
            "Use the already evaluated fallback",
            "Fallback value",
        ),
        "map_or_else" => (
            "Pass the value inside Some to the transformation",
            "Transformation result",
            "Call the fallback function",
            "Fallback result",
        ),
        "and" => (
            "Discard the value inside Some and return the second option",
            "Second Option (Some or None)",
            "Discard the already evaluated second option",
            "None",
        ),
        "and_then" => (
            "Run the callback with the value",
            "The callback's Option",
            "Skip the callback",
            "None",
        ),
        "or" => (
            "Keep the first Option and discard the already evaluated second Option",
            "Some(first value)",
            "Use the already evaluated second Option",
            "Second Option (Some or None)",
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
        "flatten" => {
            return vec![
                branch("Some(Some(value))", "Remove one layer of Option and keep the value", "Some(value)"),
                branch("Some(None)", "Remove the outer Some and keep the inner absence", "None"),
                branch("None", "Keep the absence", "None"),
            ];
        }
        "zip" => {
            return vec![
                branch("Both are Some", "Pair the two values, with the receiver's value first", "Some((first, second))"),
                branch("Either is None", "There is no pair", "None"),
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
        "as_deref_mut" => (
            "Use DerefMut to borrow the target for writing. If the option holds a mutable reference, borrow its target again.",
            "Some(mutable reference to the target)",
            "Keep the option empty",
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
        (Family::NonZero, "get") => (
            "Read the nonzero integer",
            "Return the stored integer in its underlying integer type. Its value is guaranteed to be different from zero.",
        ),
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
        (Family::Effect, "recv_optional" | "recv_mpsc") => ("Prepare to receive a message", "Await the future to receive a message. It returns None when the channel is closed and empty."),
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
    fn matches_propagation_uses_syntax_when_the_operator_has_no_type_evidence() {
        let text = "fn run() { let test = || matches!(load()?, Some(value) if guard(value)?); }";
        let tree = crate::parse(text).unwrap();
        let root = tree.root_node();
        let offset = text.find("matches!").unwrap();
        let call = root.descendant_for_byte_range(offset, offset + "matches".len()).unwrap().parent().unwrap();
        assert_eq!(call.kind(), "macro_invocation");
        let index = LineIndex::new(text);
        let src = Src { text, file: "matches.rs", index: &index };
        let evidence = Evidence {
            operations: [(range(call), Some(Resolved { family: Family::Macro, name: "matches".into(), definition: None }))].into(),
            ..Default::default()
        };
        let (readings, _, _) = collect(src, root, &evidence);
        let paths: Vec<_> = readings.iter().filter_map(|fact| match &fact.content {
            ReadingContent::Paths { branches, .. } => Some((fact, branches)),
            _ => None,
        }).collect();
        let [(fact, branches)] = paths.as_slice() else { panic!("Expected only the first argument's question mark: {paths:?}") };
        assert_eq!(fact.span.start_byte as usize, text.find('?').unwrap());
        assert_eq!(branches[0].action, "Take the value produced by ?");
        assert_eq!(branches[1].action, "Return early from this closure before testing the pattern");
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
    }

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

mod immutable_binding;
