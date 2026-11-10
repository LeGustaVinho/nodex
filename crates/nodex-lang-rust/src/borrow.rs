use std::collections::{BTreeMap, HashMap, HashSet};

use nodex_ir::{
    FlowGraph, ValueBasis, ValueInfo, ValueKind, ValueQualifier,
    borrow::{
        BorrowArgument, BorrowBinding, BorrowCall, BorrowModel, BorrowPath, BorrowRoot, BorrowUse,
    },
};
use nodex_lang_api::syntax::{Src, children, named_children};
use tree_sitter::Node;

use crate::{expr::method_call, structure::BodyJob, syntax::unparen};

type Paths = Vec<BorrowPath>;

#[derive(Clone, Default, PartialEq, Eq)]
struct State {
    bindings: Vec<BTreeMap<String, BorrowBinding>>,
    references: BTreeMap<u32, Paths>,
}

impl State {
    fn get(&self, name: &str) -> Option<&BorrowBinding> {
        self.bindings.iter().rev().find_map(|scope| scope.get(name))
    }

    fn push(&mut self) {
        self.bindings.push(BTreeMap::new());
    }

    fn pop(&mut self) {
        for binding in self
            .bindings
            .pop()
            .into_iter()
            .flat_map(|scope| scope.into_values())
        {
            self.references.remove(&binding.span.start_byte);
        }
    }

    fn bind(&mut self, binding: BorrowBinding, paths: Paths) {
        self.references.insert(binding.span.start_byte, paths);
        self.bindings
            .last_mut()
            .expect("scope")
            .insert(binding.name.clone(), binding);
    }

    fn merge(&mut self, other: &Self) {
        for (key, paths) in &mut self.references {
            if let Some(alternatives) = other.references.get(key) {
                union(paths, alternatives.clone());
            }
        }
    }

    fn with_writes_from(&self, branch: &Self) -> Self {
        let mut after = self.clone();
        for (key, paths) in &mut after.references {
            if let Some(current) = branch.references.get(key) {
                *paths = current.clone();
            }
        }
        after
    }
}

fn union(paths: &mut Paths, more: Paths) {
    for path in more {
        if !paths.contains(&path) {
            if paths.len() >= 32 {
                let unknown = BorrowPath::unknown();
                if !paths.contains(&unknown) {
                    paths.push(unknown);
                }
                break;
            }
            paths.push(path);
        }
    }
}

fn through(mut paths: Paths, binding: &BorrowBinding) -> Paths {
    for path in &mut paths {
        if !path.through.contains(binding) {
            path.through.push(binding.clone());
        }
    }
    paths
}

fn is_bare_break(node: Node) -> bool {
    node.kind() == "break_expression"
        && named_children(node)
            .iter()
            .all(|child| child.is_extra() || child.kind() == "label")
}

#[derive(Default)]
struct ControlTarget<'a> {
    label: Option<&'a str>,
    is_loop: bool,
    breaks: Option<State>,
    break_values: Paths,
    continues: Option<State>,
}

#[derive(Default)]
struct Condition {
    yes: Option<State>,
    no: Option<State>,
}

#[derive(Clone)]
struct DirectFields<'tree> {
    fields: HashMap<String, FieldSource<'tree>>,
    by_reference: bool,
}

#[derive(Clone, Copy)]
enum FieldSource<'tree> {
    Type(Node<'tree>),
    Literal(Node<'tree>),
    FieldOnly,
}

struct Analysis<'a, 'tree> {
    src: Src<'a>,
    model: BorrowModel,
    uses: BTreeMap<(u32, u32), BorrowUse>,
    calls: BTreeMap<(u32, u32), BorrowCall>,
    kinds: HashMap<u32, (ValueKind, Option<ValueQualifier>)>,
    builtin_index: HashSet<u32>,
    direct_fields: HashMap<u32, DirectFields<'tree>>,
    targets: Vec<ControlTarget<'a>>,
}

pub(crate) fn analyze(src: Src, job: &BodyJob<'_>, flow: &FlowGraph) -> BorrowModel {
    let mut analysis = Analysis {
        src,
        model: BorrowModel::default(),
        uses: BTreeMap::new(),
        calls: BTreeMap::new(),
        kinds: flow
            .nodes
            .iter()
            .flat_map(|n| &n.outputs)
            .filter_map(|pin| {
                let site = pin.declaration.as_ref()?;
                let info = pin.value_info?;
                Some((site.start_byte, (info.kind, info.qualifier)))
            })
            .collect(),
        builtin_index: flow
            .nodes
            .iter()
            .flat_map(|node| &node.outputs)
            .filter_map(|pin| {
                let span = pin.declaration.as_ref()?;
                builtin_index(pin.ty.as_deref()?).then_some(span.start_byte)
            })
            .collect(),
        direct_fields: HashMap::new(),
        targets: Vec::new(),
    };
    let mut state = State::default();
    state.push();
    if let Some(parameters) = job.decl.child_by_field_name("parameters") {
        for parameter in named_children(parameters) {
            let name = if parameter.kind() == "self_parameter" {
                children(parameter).into_iter().find(|n| n.kind() == "self")
            } else {
                parameter
                    .child_by_field_name("pattern")
                    .or_else(|| (parameter.kind() == "identifier").then_some(parameter))
            };
            let binding = name.and_then(|name| analysis.simple_binding(name));
            let index = analysis.model.parameters.len();
            analysis.model.parameters.push(binding.clone());
            if let Some(binding) = binding {
                let ty = parameter.child_by_field_name("type").map(crate::syntax::unparen_type);
                let is_ref = ty.is_some_and(|t| t.kind() == "reference_type")
                    || (parameter.kind() == "self_parameter"
                        && children(parameter).iter().any(|n| n.kind() == "&"));
                let fields = if parameter.kind() == "self_parameter" {
                    analysis.receiver_fields(parameter, is_ref)
                } else {
                    ty.and_then(|ty| analysis.fields_of_type(ty))
                };
                if let Some(fields) = fields {
                    analysis
                        .direct_fields
                        .insert(binding.span.start_byte, fields);
                }
                let mut paths = Vec::new();
                if is_ref {
                    let mut path = BorrowPath::from(BorrowRoot::Parameter(index));
                    path.mutable = ty
                        .unwrap_or(parameter)
                        .child_by_field_name("mutable")
                        .is_some()
                        || children(ty.unwrap_or(parameter))
                            .iter()
                            .any(|n| n.kind() == "mutable_specifier");
                    paths.push(path);
                }
                let paths = through(paths, &binding);
                analysis.record(binding.span.clone(), paths.clone());
                state.bind(binding, paths);
            }
        }
    }
    if let Some(paths) = analysis.eval(job.body, &mut state) {
        union(&mut analysis.model.returns, paths);
    }
    analysis.model.uses = analysis.uses.into_values().collect();
    analysis.model.calls = analysis.calls.into_values().collect();
    analysis.model
}

pub(crate) fn annotate_control_values(flow: &mut FlowGraph, model: &BorrowModel) {
    let mut changed = false;
    for node in flow
        .nodes
        .iter_mut()
        .filter(|node| node.flavors.iter().any(|flavor| flavor == "block-value"))
    {
        for pin in node
            .outputs
            .iter_mut()
            .filter(|pin| pin.ty.is_none() && pin.value_info.is_none())
        {
            let Some(declaration) = &pin.declaration else {
                continue;
            };
            let Ok(site) = model
                .uses
                .binary_search_by_key(&(declaration.start_byte, declaration.end_byte), |site| {
                    (site.span.start_byte, site.span.end_byte)
                })
            else {
                continue;
            };
            let paths = &model.uses[site].paths;
            if paths.is_empty()
                || paths.iter().any(|path| {
                    matches!(
                        path.root,
                        BorrowRoot::Call(_) | BorrowRoot::TypedPlace { .. } | BorrowRoot::Unknown
                    )
                })
            {
                continue;
            }
            let (kind, qualifier) = if paths.iter().any(|path| path.root == BorrowRoot::RawPointer)
            {
                (ValueKind::Pointer, Some(ValueQualifier::Raw))
            } else {
                let qualifier = if paths.iter().all(|path| path.mutable) {
                    Some(ValueQualifier::ExclusiveBorrow)
                } else if paths.iter().all(|path| !path.mutable) {
                    Some(ValueQualifier::SharedBorrow)
                } else {
                    None
                };
                (ValueKind::Reference, qualifier)
            };
            pin.value_info = Some(ValueInfo {
                kind,
                basis: ValueBasis::Syntax,
                qualifier,
                passing: None,
            });
            changed = true;
        }
    }
    if changed {
        nodex_lang_api::value_shapes::carry_to_uses(flow);
    }
}

fn builtin_index(ty: &str) -> bool {
    let ty = ty.trim();
    let referent = ty
        .strip_prefix('&')
        .map(|text| {
            let text = text.trim_start();
            let text = if text.starts_with('\'') {
                text.split_once(char::is_whitespace)
                    .map_or("", |(_, rest)| rest.trim_start())
            } else {
                text
            };
            text.strip_prefix("mut ").unwrap_or(text).trim_start()
        })
        .unwrap_or(ty);
    referent.starts_with('[')
        || referent.starts_with("std::vec::Vec<")
        || referent.starts_with("alloc::vec::Vec<")
}

impl<'a, 'tree> Analysis<'a, 'tree> {
    fn label(&self, n: Node) -> Option<&'a str> {
        named_children(n)
            .into_iter()
            .find(|child| child.kind() == "label")
            .and_then(|label| named_children(label).into_iter().next())
            .map(|name| self.src.text(name))
    }

    fn receiver_fields(
        &self,
        receiver: Node<'tree>,
        by_reference: bool,
    ) -> Option<DirectFields<'tree>> {
        let owner = std::iter::successors(receiver.parent(), Node::parent)
            .find(|ancestor| ancestor.kind() == "impl_item")?;
        let ty = owner.child_by_field_name("type")?;
        if ty.end_byte() > receiver.start_byte() {
            return None;
        }
        let mut fields = self.fields_of_type(ty)?;
        if by_reference && fields.by_reference {
            return None;
        }
        fields.by_reference |= by_reference;
        Some(fields)
    }

    fn fields_of_type(&self, ty: Node<'tree>) -> Option<DirectFields<'tree>> {
        let by_reference = ty.kind() == "reference_type";
        let mut base = if by_reference {
            ty.child_by_field_name("type")?
        } else {
            ty
        };
        if base.kind() == "tuple_type" {
            let types = named_children(base);
            if types.len() == 1 && !children(base).iter().any(|child| child.kind() == ",") {
                let mut fields = self.fields_of_type(types[0])?;
                if by_reference && fields.by_reference {
                    return None;
                }
                fields.by_reference |= by_reference;
                return Some(fields);
            }
            return Some(DirectFields {
                fields: types
                    .into_iter()
                    .enumerate()
                    .map(|(i, ty)| (i.to_string(), FieldSource::Type(ty)))
                    .collect(),
                by_reference,
            });
        }
        if base.kind() == "generic_type" {
            base = base.child_by_field_name("type")?;
        }
        if base.kind() != "type_identifier" {
            return None;
        }
        let name = self.src.text(base);
        if name == "Self" {
            return self.receiver_fields(ty, by_reference);
        }
        let mut ancestor = ty.parent();
        while let Some(scope) = ancestor {
            ancestor = scope.parent();
            if scope
                .child_by_field_name("type_parameters")
                .is_some_and(|params| {
                    named_children(params).iter().any(|param| {
                        param
                            .child_by_field_name("name")
                            .is_some_and(|n| self.src.text(n) == name)
                    })
                })
            {
                return None;
            }
            if !matches!(scope.kind(), "source_file" | "block" | "declaration_list") {
                continue;
            }
            let children = named_children(scope);
            let mut items = children.iter().copied().filter(|item| {
                matches!(
                    item.kind(),
                    "struct_item" | "union_item" | "enum_item" | "type_item" | "trait_item"
                ) && item
                    .child_by_field_name("name")
                    .is_some_and(|n| self.src.text(n) == name)
            });
            let Some(item) = items.next() else {
                // A module or import can change what an unqualified type name denotes.
                if scope
                    .parent()
                    .is_some_and(|parent| parent.kind() == "mod_item")
                    || children
                        .iter()
                        .any(|child| child.kind() == "use_declaration")
                {
                    return None;
                }
                continue;
            };
            if items.next().is_some()
                || !matches!(item.kind(), "struct_item" | "union_item")
                || item
                    .prev_named_sibling()
                    .is_some_and(|prev| prev.kind() == "attribute_item")
            {
                return None;
            }
            let body = item.child_by_field_name("body")?;
            let fields = named_children(body);
            let fields = if body.kind() == "ordered_field_declaration_list" {
                // Attributes can conditionally remove tuple fields and change their indices.
                if fields.iter().any(|field| field.kind() == "attribute_item") {
                    return None;
                }
                fields
                    .into_iter()
                    .filter(|field| field.kind() != "visibility_modifier")
                    .enumerate()
                    .map(|(i, ty)| (i.to_string(), FieldSource::Type(ty)))
                    .collect()
            } else {
                fields
                    .iter()
                    .filter_map(|field| {
                        let name = field.child_by_field_name("name")?;
                        let ty = field.child_by_field_name("type")?;
                        (!field
                            .prev_named_sibling()
                            .is_some_and(|prev| prev.kind() == "attribute_item"))
                        .then(|| (self.src.text(name).to_owned(), FieldSource::Type(ty)))
                    })
                    .collect()
            };
            return Some(DirectFields {
                fields,
                by_reference,
            });
        }
        None
    }

    fn fields_of_value(&self, value: Node<'tree>, state: &State) -> Option<DirectFields<'tree>> {
        let value = unparen(value);
        let fields = match value.kind() {
            "struct_expression" => named_children(value.child_by_field_name("body")?)
                .into_iter()
                .filter(|field| {
                    !named_children(*field)
                        .iter()
                        .any(|child| child.kind() == "attribute_item")
                })
                .filter_map(|field| {
                    let name = if field.kind() == "shorthand_field_initializer" {
                        named_children(field)
                            .into_iter()
                            .find(|child| child.kind() == "identifier")
                    } else {
                        field.child_by_field_name("field")
                    }?;
                    let source = field
                        .child_by_field_name("value")
                        .map_or(FieldSource::FieldOnly, FieldSource::Literal);
                    Some((self.src.text(name).to_owned(), source))
                })
                .collect(),
            "tuple_expression" => named_children(value)
                .into_iter()
                .filter(|child| child.kind() != "attribute_item")
                .enumerate()
                .map(|(i, value)| (i.to_string(), FieldSource::Literal(value)))
                .collect(),
            "identifier" | "self" => {
                let binding = state.get(self.src.text(value))?;
                return self.direct_fields.get(&binding.span.start_byte).cloned();
            }
            "reference_expression"
                if !children(value).iter().any(|child| child.kind() == "raw") =>
            {
                let mut fields =
                    self.fields_of_value(value.child_by_field_name("value")?, state)?;
                if fields.by_reference {
                    return None;
                }
                fields.by_reference = true;
                return Some(fields);
            }
            "field_expression" => {
                let base = value.child_by_field_name("value")?;
                let name = self.src.text(value.child_by_field_name("field")?);
                let fields = self.fields_of_value(base, state)?;
                let fields = match fields.fields.get(name)? {
                    FieldSource::Type(ty) => self.fields_of_type(*ty),
                    FieldSource::Literal(literal)
                        if matches!(
                            unparen(*literal).kind(),
                            "struct_expression" | "tuple_expression"
                        ) =>
                    {
                        self.fields_of_value(*literal, state)
                    }
                    _ => None,
                }?;
                // A reference stored inside a field can point outside the containing place.
                return (!fields.by_reference).then_some(fields);
            }
            _ => return None,
        };
        Some(DirectFields {
            fields,
            by_reference: false,
        })
    }

    fn simple_binding(&self, n: Node) -> Option<BorrowBinding> {
        let n = if n.kind() == "mut_pattern" {
            named_children(n).into_iter().last()?
        } else {
            n
        };
        matches!(
            n.kind(),
            "identifier" | "self" | "shorthand_field_identifier"
        )
        .then(|| BorrowBinding {
            name: self.src.text(n).to_owned(),
            span: self.src.span(n),
        })
    }

    fn record(&mut self, span: nodex_ir::Span, paths: Paths) {
        if !paths.is_empty() {
            union(
                &mut self
                    .uses
                    .entry((span.start_byte, span.end_byte))
                    .or_insert_with(|| BorrowUse {
                        span,
                        paths: Vec::new(),
                    })
                    .paths,
                paths,
            );
        }
    }

    fn place(&self, n: Node<'tree>, state: &State) -> Paths {
        let n = unparen(n);
        match n.kind() {
            "identifier" | "self" => state.get(self.src.text(n)).map_or_else(
                || vec![BorrowPath::unknown()],
                |binding| vec![BorrowPath::from(BorrowRoot::Local(binding.clone()))],
            ),
            "unary_expression" if children(n).iter().any(|c| c.kind() == "*") => {
                let Some(operand) = named_children(n).into_iter().next() else {
                    return vec![BorrowPath::unknown()];
                };
                self.referent(operand, state)
            }
            "field_expression" | "index_expression" => {
                let Some(base) = named_children(n).into_iter().next() else {
                    return vec![BorrowPath::unknown()];
                };
                let base_end = base.end_byte();
                let base = unparen(base);
                let mut paths = if n.kind() == "field_expression" {
                    let Some(fields) = self.fields_of_value(base, state).filter(|fields| {
                        n.child_by_field_name("field")
                            .is_some_and(|field| fields.fields.contains_key(self.src.text(field)))
                    }) else {
                        // Field lookup can implicitly call a custom Deref.
                        return vec![BorrowPath::unknown()];
                    };
                    if fields.by_reference {
                        state
                            .get(self.src.text(base))
                            .and_then(|binding| state.references.get(&binding.span.start_byte))
                            .filter(|paths| !paths.is_empty())
                            .cloned()
                            .unwrap_or_else(|| vec![BorrowPath::unknown()])
                    } else {
                        self.place(base, state)
                    }
                } else {
                    let Some(binding) = state.get(self.src.text(base)) else {
                        return vec![BorrowPath::unknown()];
                    };
                    let key = binding.span.start_byte;
                    if !self.builtin_index.contains(&key) {
                        // A custom Index implementation can return a reference from elsewhere.
                        return vec![BorrowPath::unknown()];
                    }
                    if self
                        .kinds
                        .get(&key)
                        .is_some_and(|(kind, _)| *kind == ValueKind::Value)
                    {
                        self.place(base, state)
                    } else {
                        self.referent(base, state)
                    }
                };
                // A field's name or an index expression is a projection, not a separate owner.
                let suffix = self.src.text.get(base_end..n.end_byte()).unwrap_or("");
                for path in &mut paths {
                    if path.projection.len() + suffix.len() <= 200 {
                        path.projection.push_str(suffix);
                    }
                }
                paths
            }
            "scoped_identifier" | "macro_invocation" => vec![BorrowPath::unknown()],
            _ => vec![BorrowPath::from(BorrowRoot::Temporary(self.src.span(n)))],
        }
    }

    fn referent(&self, n: Node, state: &State) -> Paths {
        let n = unparen(n);
        if let Some(binding) = state.get(self.src.text(n)) {
            match self.kinds.get(&binding.span.start_byte) {
                Some((ValueKind::Pointer, Some(ValueQualifier::Unique))) => {
                    return vec![BorrowPath::from(BorrowRoot::Local(binding.clone()))];
                }
                Some((ValueKind::Pointer, Some(ValueQualifier::Shared))) => {
                    return vec![BorrowPath::from(BorrowRoot::Shared(binding.clone()))];
                }
                Some((ValueKind::Pointer, _)) => return vec![BorrowPath::unknown()],
                _ => {}
            }
            let paths = state
                .references
                .get(&binding.span.start_byte)
                .cloned()
                .unwrap_or_default();
            if self
                .kinds
                .get(&binding.span.start_byte)
                .is_some_and(|(kind, _)| *kind == ValueKind::Reference)
            {
                return paths;
            }
            if self
                .kinds
                .get(&binding.span.start_byte)
                .is_none_or(|(kind, _)| *kind == ValueKind::Unknown)
            {
                return vec![BorrowPath::from(BorrowRoot::TypedPlace {
                    binding: binding.clone(),
                    fallback: paths,
                })];
            }
        }
        vec![BorrowPath::unknown()]
    }

    /// None means this execution path has already left the current block.
    fn eval(&mut self, n: Node<'tree>, state: &mut State) -> Option<Paths> {
        let n = unparen(n);
        // The grammar can parse `break &place` as an infix expression with a bare
        // break on the left. Rust parses the ampersand as part of the break value.
        if n.kind() == "binary_expression"
            && let Some(jump) = n.child_by_field_name("left")
            && is_bare_break(jump)
            && let Some(operator) = n.child_by_field_name("operator")
            && matches!(operator.kind(), "&" | "&&")
        {
            let right = n.child_by_field_name("right")?;
            let mutable = self.src.text(right) == "mut"
                || children(n).iter().any(|child| {
                    child.is_error()
                        && child.start_byte() >= operator.end_byte()
                        && child.end_byte() <= right.start_byte()
                        && self.src.text(*child).trim() == "mut"
                });
            let recovery = n
                .parent()
                .filter(|parent| parent.kind() == "expression_statement")
                .and_then(|parent| {
                    named_children(parent)
                        .into_iter()
                        .find(|sibling| sibling.start_byte() >= n.end_byte() && sibling.is_error())
                });
            let mut paths = match (operator.kind(), self.src.text(right)) {
                ("&", "raw") if recovery.is_some() => {
                    vec![BorrowPath::from(BorrowRoot::RawPointer)]
                }
                ("&", "mut") => {
                    // For a bare identifier the grammar puts the operand in an error sibling.
                    let place = recovery
                        .filter(|error| error.named_child_count() == 1)
                        .and_then(|error| error.named_child(0))
                        .filter(|operand| matches!(operand.kind(), "identifier" | "self"));
                    place.map_or_else(
                        || vec![BorrowPath::unknown()],
                        |place| self.place(place, state),
                    )
                }
                ("&", _) if right.kind() == "type_cast_expression" => {
                    let operand = right.child_by_field_name("value")?;
                    self.eval(operand, state)?;
                    match right.child_by_field_name("type").map(|ty| ty.kind()) {
                        Some("reference_type") => self.place(operand, state),
                        Some("pointer_type") => vec![BorrowPath::from(BorrowRoot::RawPointer)],
                        _ => vec![BorrowPath::unknown()],
                    }
                }
                ("&", _)
                    if matches!(right.kind(), "call_expression" | "binary_expression")
                        && self
                            .src
                            .text(right)
                            .strip_prefix("mut")
                            .is_some_and(|rest| {
                                rest.starts_with(char::is_whitespace) || rest.starts_with('(')
                            }) =>
                {
                    vec![BorrowPath::unknown()]
                }
                ("&", _) if !right.has_error() => {
                    self.eval(right, state)?;
                    self.place(right, state)
                }
                _ => vec![BorrowPath::unknown()],
            };
            for path in &mut paths {
                path.mutable = mutable;
            }
            self.jump(jump, state, paths);
            return None;
        }
        let paths = match n.kind() {
            "block" => {
                let label = self.label(n);
                if let Some(label) = label {
                    self.targets.push(ControlTarget {
                        label: Some(label),
                        ..ControlTarget::default()
                    });
                }
                state.push();
                let mut result = Some(Vec::new());
                for child in named_children(n)
                    .into_iter()
                    .filter(|child| !child.is_extra() && child.kind() != "label")
                {
                    result = self.eval(child, state);
                    if result.is_none() {
                        break;
                    }
                }
                state.pop();
                if label.is_some() {
                    let target = self.targets.pop().expect("labeled block");
                    let mut exit = result.as_ref().map(|_| state.clone());
                    let mut paths = result.unwrap_or_default();
                    if let Some(branch) = target.breaks {
                        merge_state(&mut exit, state.with_writes_from(&branch));
                    }
                    union(&mut paths, target.break_values);
                    *state = exit?;
                    paths
                } else {
                    result?
                }
            }
            "let_declaration" => {
                let value = n.child_by_field_name("value");
                let paths = if let Some(value) = value {
                    self.eval(value, state)?
                } else {
                    Vec::new()
                };
                if let Some(alternative) = n.child_by_field_name("alternative") {
                    self.eval(alternative, &mut state.clone());
                }
                if let Some(pattern) = n.child_by_field_name("pattern") {
                    if let Some(binding) = self.simple_binding(pattern) {
                        let fields = match n.child_by_field_name("type") {
                            Some(ty) => self.fields_of_type(ty),
                            None => value.and_then(|value| self.fields_of_value(value, state)),
                        };
                        if let Some(fields) = fields {
                            self.direct_fields.insert(binding.span.start_byte, fields);
                        }
                    }
                    self.bind_pattern(pattern, value, paths, state);
                }
                Vec::new()
            }
            "reference_expression" => {
                let value = n.child_by_field_name("value")?;
                self.eval(value, state)?;
                if children(n).iter().any(|c| c.kind() == "raw") {
                    return Some(vec![BorrowPath::from(BorrowRoot::RawPointer)]);
                }
                let mutable = children(n).iter().any(|c| c.kind() == "mutable_specifier");
                let mut paths = self.place(value, state);
                for path in &mut paths {
                    path.mutable = mutable;
                }
                paths
            }
            "type_cast_expression"
                if n.child_by_field_name("type")
                    .is_some_and(|ty| ty.kind() == "pointer_type") =>
            {
                self.eval(n.child_by_field_name("value")?, state)?;
                vec![BorrowPath::from(BorrowRoot::RawPointer)]
            }
            "identifier" | "self" => state
                .get(self.src.text(n))
                .map_or_else(Vec::new, |binding| {
                    let key = binding.span.start_byte;
                    if self.kinds.get(&key)
                        == Some(&(ValueKind::Pointer, Some(ValueQualifier::Raw)))
                    {
                        vec![BorrowPath::from(BorrowRoot::RawPointer)]
                    } else {
                        state.references.get(&key).cloned().unwrap_or_default()
                    }
                }),
            "assignment_expression" => {
                let left = n.child_by_field_name("left")?;
                let right = n.child_by_field_name("right")?;
                let paths = self.eval(right, state)?;
                if let Some(binding) = state.get(self.src.text(left)).cloned() {
                    let paths = through(paths, &binding);
                    self.record(self.src.span(left), paths.clone());
                    state.references.insert(binding.span.start_byte, paths);
                } else if left.kind() == "unary_expression" {
                    self.eval(left, state)?;
                    self.invalidate_slots(self.place(left, state), state);
                }
                Vec::new()
            }
            "if_expression" => self.branches(n, state)?,
            "match_expression" => self.match_arms(n, state)?,
            "binary_expression"
                if n.child_by_field_name("operator")
                    .is_some_and(|operator| matches!(operator.kind(), "&&" | "||")) =>
            {
                let Condition { mut yes, no } = self.condition(n, state.clone());
                if let Some(no) = no {
                    merge_state(&mut yes, no);
                }
                *state = yes?;
                Vec::new()
            }
            "loop_expression" | "while_expression" | "for_expression" => {
                self.loop_body(n, state)?
            }
            "return_expression" => {
                if let Some(value) = named_children(n).into_iter().next() {
                    let paths = self.eval(value, state)?;
                    union(&mut self.model.returns, paths);
                }
                return None;
            }
            "break_expression" | "continue_expression" => {
                let paths = if let Some(value) =
                    named_children(n).into_iter().find(|c| c.kind() != "label")
                {
                    self.eval(value, state)?
                } else {
                    Vec::new()
                };
                self.jump(n, state, paths);
                return None;
            }
            "call_expression" => self.call(n, state)?,
            "await_expression" => {
                let future = named_children(n).into_iter().find(|child| !child.is_extra())?;
                self.eval(future, state)?
            }
            "closure_expression" | "function_item" | "function_signature_item" => Vec::new(),
            "string_literal" | "raw_string_literal" => {
                vec![BorrowPath::from(BorrowRoot::Static(self.src.span(n)))]
            }
            "expression_statement" => {
                let value = named_children(n).into_iter().next()?;
                let paths = self.eval(value, state)?;
                if children(n).iter().any(|child| child.kind() == ";") {
                    Vec::new()
                } else {
                    paths
                }
            }
            _ => {
                for child in named_children(n) {
                    self.eval(child, state)?;
                }
                Vec::new()
            }
        };
        // References can coerce to a raw pointer at a control-flow join. The resulting
        // value has no borrow origin, even on the path that supplied a reference.
        let paths = if paths.iter().any(|path| path.root == BorrowRoot::RawPointer) {
            vec![BorrowPath::from(BorrowRoot::RawPointer)]
        } else {
            paths
        };
        self.record(self.src.span(n), paths.clone());
        Some(paths)
    }

    fn jump(&mut self, n: Node, state: &State, paths: Paths) {
        let label = self.label(n);
        if let Some(target) = self.targets.iter_mut().rev().find(|target| match label {
            Some(label) => target.label == Some(label),
            None => target.is_loop,
        }) {
            if n.kind() == "break_expression" {
                merge_state(&mut target.breaks, state.clone());
                union(&mut target.break_values, paths);
            } else {
                merge_state(&mut target.continues, state.clone());
            }
        }
    }

    fn bind_pattern(
        &mut self,
        pattern: Node,
        value: Option<Node<'tree>>,
        paths: Paths,
        state: &mut State,
    ) {
        if let Some(binding) = self.simple_binding(pattern) {
            let paths = through(paths, &binding);
            self.record(binding.span.clone(), paths.clone());
            state.bind(binding, paths);
        } else if pattern.kind() == "ref_pattern" {
            let Some(binding) = named_children(pattern)
                .into_iter()
                .last()
                .and_then(|n| self.simple_binding(n))
            else {
                return;
            };
            let mut paths = value.map_or_else(
                || vec![BorrowPath::unknown()],
                |value| self.place(value, state),
            );
            let mutable = self.src.text(pattern).split_whitespace().nth(1) == Some("mut");
            for path in &mut paths {
                path.mutable = mutable;
            }
            let paths = through(paths, &binding);
            self.record(binding.span.clone(), paths.clone());
            state.bind(binding, paths);
        } else {
            // Keep lexical identity for destructuring even when a component's source is unknown.
            for child in named_children(pattern) {
                self.bind_pattern(child, None, vec![BorrowPath::unknown()], state);
            }
        }
    }

    fn condition(&mut self, n: Node<'tree>, mut state: State) -> Condition {
        let n = unparen(n);
        match n.kind() {
            "boolean_literal" => {
                return if self.src.text(n) == "true" {
                    Condition {
                        yes: Some(state),
                        no: None,
                    }
                } else {
                    Condition {
                        yes: None,
                        no: Some(state),
                    }
                };
            }
            "binary_expression" => {
                if let (Some(left), Some(right), Some(operator)) = (
                    n.child_by_field_name("left"),
                    n.child_by_field_name("right"),
                    n.child_by_field_name("operator"),
                ) {
                    match operator.kind() {
                        "&&" if !is_bare_break(left) => {
                            let Condition { yes, mut no } = self.condition(left, state);
                            let right = yes.map_or_else(Condition::default, |state| {
                                self.condition(right, state)
                            });
                            if let Some(skipped) = right.no {
                                merge_state(&mut no, skipped);
                            }
                            return Condition { yes: right.yes, no };
                        }
                        "||" => {
                            let Condition { mut yes, no } = self.condition(left, state);
                            let right = no.map_or_else(Condition::default, |state| {
                                self.condition(right, state)
                            });
                            if let Some(passed) = right.yes {
                                merge_state(&mut yes, passed);
                            }
                            return Condition { yes, no: right.no };
                        }
                        _ => {}
                    }
                } else if children(n)
                    .iter()
                    .any(|child| matches!(child.kind(), "&&" | "||"))
                {
                    return Condition::default();
                }
            }
            "unary_expression" if children(n).iter().any(|child| child.kind() == "!") => {
                if let Some(operand) = named_children(n)
                    .into_iter()
                    .find(|child| !child.is_extra())
                {
                    let Condition { yes, no } = self.condition(operand, state);
                    return Condition { yes: no, no: yes };
                }
            }
            "block" if self.label(n).is_none() => {
                let statements: Vec<_> = named_children(n)
                    .into_iter()
                    .filter(|child| !child.is_extra())
                    .collect();
                if let Some((tail, prefix)) = statements.split_last() {
                    state.push();
                    for statement in prefix {
                        if self.eval(*statement, &mut state).is_none() {
                            return Condition::default();
                        }
                    }
                    let mut outcomes = self.condition(*tail, state);
                    for state in outcomes.yes.iter_mut().chain(outcomes.no.iter_mut()) {
                        state.pop();
                    }
                    return outcomes;
                }
            }
            "expression_statement" if !children(n).iter().any(|child| child.kind() == ";") => {
                if let Some(value) = named_children(n)
                    .into_iter()
                    .find(|child| !child.is_extra())
                {
                    return self.condition(value, state);
                }
            }
            _ => {}
        }
        if self.eval(n, &mut state).is_some() {
            Condition {
                yes: Some(state.clone()),
                no: Some(state),
            }
        } else {
            Condition::default()
        }
    }

    fn branches(&mut self, n: Node<'tree>, state: &mut State) -> Option<Paths> {
        let Condition { yes, no } =
            self.condition(n.child_by_field_name("condition")?, state.clone());
        let consequence = n.child_by_field_name("consequence")?;
        let alternative = n.child_by_field_name("alternative").and_then(|alt| {
            named_children(alt)
                .into_iter()
                .find(|child| !child.is_extra())
        });
        let mut joined = None;
        let mut paths = Vec::new();
        for (branch, body) in [(yes, Some(consequence)), (no, alternative)] {
            if let Some(mut branch) = branch
                && let Some(result) =
                    body.map_or(Some(Vec::new()), |body| self.eval(body, &mut branch))
            {
                merge_state(&mut joined, branch);
                union(&mut paths, result);
            }
        }
        *state = joined?;
        Some(paths)
    }

    fn match_arms(&mut self, n: Node<'tree>, state: &mut State) -> Option<Paths> {
        let value = n.child_by_field_name("value")?;
        let paths = self.eval(value, state)?;
        let mut joined: Option<State> = None;
        let mut result = Vec::new();
        for arm in named_children(n.child_by_field_name("body")?) {
            let mut branch = state.clone();
            branch.push();
            if let Some(pattern) = arm.child_by_field_name("pattern") {
                if let Some(p) = named_children(pattern).into_iter().next() {
                    self.bind_pattern(p, Some(value), paths.clone(), &mut branch);
                }
                if let Some(guard) = pattern.child_by_field_name("condition") {
                    let Condition { yes, no } = self.condition(guard, branch);
                    // A failed guard can carry writes into the following arm.
                    if let Some(no) = no {
                        state.merge(&no);
                    }
                    let Some(yes) = yes else { continue };
                    branch = yes;
                }
            }
            let Some(value) = arm.child_by_field_name("value") else {
                continue;
            };
            let Some(paths) = self.eval(value, &mut branch) else {
                continue;
            };
            branch.pop();
            union(&mut result, paths);
            if let Some(joined) = &mut joined {
                joined.merge(&branch);
            } else {
                joined = Some(branch);
            }
        }
        *state = joined?;
        Some(result)
    }

    fn loop_body(&mut self, n: Node<'tree>, state: &mut State) -> Option<Paths> {
        let body = n.child_by_field_name("body")?;
        if n.kind() == "for_expression" {
            self.eval(n.child_by_field_name("value")?, state)?;
        }
        let original = state.clone();
        let mut header = original.clone();
        let mut exit = None;
        let mut result = Vec::new();
        let label = self.label(n);
        for iteration in 0..=8 {
            if iteration == 8 {
                for paths in header
                    .references
                    .values_mut()
                    .filter(|paths| !paths.is_empty())
                {
                    union(paths, vec![BorrowPath::unknown()]);
                }
            }
            self.targets.push(ControlTarget {
                label,
                is_loop: true,
                ..ControlTarget::default()
            });
            let Condition { yes, no } = if let Some(condition) = n.child_by_field_name("condition")
            {
                self.condition(condition, header.clone())
            } else {
                Condition {
                    yes: Some(header.clone()),
                    no: (n.kind() == "for_expression").then(|| header.clone()),
                }
            };
            if let Some(no) = no {
                merge_state(&mut exit, no);
            }
            let body_end = yes.and_then(|mut state| {
                state.push();
                if let Some(pattern) = n.child_by_field_name("pattern") {
                    self.bind_pattern(pattern, None, vec![BorrowPath::unknown()], &mut state);
                }
                self.eval(body, &mut state)?;
                state.pop();
                Some(state)
            });
            let ControlTarget {
                breaks,
                break_values,
                continues: next,
                ..
            } = self.targets.pop().expect("loop");
            let mut next_header = original.clone();
            if let Some(body_state) = body_end {
                next_header.merge(&body_state);
            }
            if let Some(branch) = next {
                next_header.merge(&branch);
            }
            if let Some(branch) = breaks {
                merge_state(&mut exit, original.with_writes_from(&branch));
            }
            union(&mut result, break_values);
            if next_header == header || iteration == 8 {
                break;
            }
            header = next_header;
        }
        *state = exit?;
        Some(result)
    }

    fn call(&mut self, n: Node<'tree>, state: &mut State) -> Option<Paths> {
        let span = self.src.span(n);
        let mut read = |e| -> Option<BorrowArgument> {
            let value = self.eval(e, state)?;
            let place = self.place(e, state);
            Some(BorrowArgument { value, place })
        };
        let receiver = if let Some((receiver, _)) = method_call(n) {
            Some(read(receiver)?)
        } else {
            None
        };
        let mut arguments = Vec::new();
        if let Some(args) = n.child_by_field_name("arguments") {
            for arg in named_children(args) {
                arguments.push(read(arg)?);
            }
        }
        let call = self
            .calls
            .entry((span.start_byte, span.end_byte))
            .or_insert_with(|| BorrowCall {
                span: span.clone(),
                receiver: receiver.clone(),
                arguments: arguments.clone(),
            });
        for (stored, current) in call
            .receiver
            .iter_mut()
            .chain(&mut call.arguments)
            .zip(receiver.iter().chain(&arguments))
        {
            union(&mut stored.value, current.value.clone());
            union(&mut stored.place, current.place.clone());
        }
        let mutable = receiver
            .into_iter()
            .chain(arguments)
            .flat_map(|argument| argument.value)
            .filter(|path| path.mutable)
            .collect();
        self.invalidate_slots(mutable, state);
        Some(vec![BorrowPath::from(BorrowRoot::Call(span))])
    }

    fn invalidate_slots(&self, paths: Paths, state: &mut State) {
        for path in paths {
            if let BorrowRoot::Local(binding) = path.root
                && path.projection.is_empty()
                && let Some(current) = state.references.get_mut(&binding.span.start_byte)
                && !current.is_empty()
            {
                *current = vec![BorrowPath::unknown()];
            }
        }
    }
}

fn merge_state(slot: &mut Option<State>, state: State) {
    if let Some(previous) = slot {
        previous.merge(&state);
    } else {
        *slot = Some(state);
    }
}
