//! Exact compiler types and collection usage proven by resolved calls.

use std::collections::{HashMap, HashSet};

use nodex_ir::semantic::{BindingUseFact, LocalTypeFact, RefKind, SemanticRef, ValueFactTarget};
use nodex_ir::types::{
    CollectionKind, CollectionRole, RoleEvidence, TypeInfo, TypeOrigin, TypeShape, native_shape,
};
use nodex_ir::{BindingForm, Concept, Expr, FlowEdgeKind, FlowGraph, Language, Span};

use crate::semantic::TypeFactSites;

type ScriptReads = HashMap<String, Option<TypeShape>>;

fn dynamic_script_reads(flow: &FlowGraph, facts: &TypeFactSites) -> HashMap<String, ScriptReads> {
    if flow.language != Language::Typescript
        || !flow
            .span
            .file
            .rsplit_once('.')
            .is_some_and(|(_, ext)| matches!(ext, "js" | "jsx" | "mjs" | "cjs"))
    {
        return HashMap::new();
    }
    let reassigned: HashSet<_> = flow
        .nodes
        .iter()
        .flat_map(|node| &node.binding_writes)
        .map(|binding| (binding.node.as_str(), binding.pin.as_str()))
        .collect();
    let mut definitions = HashMap::new();
    for node in &flow.nodes {
        for pin in &node.outputs {
            let key = (node.id.as_str(), pin.id.as_str());
            if !reassigned.contains(&key) {
                continue;
            }
            let declared = pin
                .declaration
                .as_ref()
                .and_then(|span| {
                    facts.get(&(span.start_byte, span.end_byte, ValueFactTarget::Binding))
                })
                .or(pin.type_info.as_ref())
                .is_some_and(|info| info.origin == TypeOrigin::Declared);
            if !declared {
                definitions.insert(key, pin.type_info.as_ref().map(|info| &info.shape));
            }
        }
    }
    let mut reads: HashMap<String, ScriptReads> = HashMap::new();
    for edge in &flow.edges {
        if edge.kind == FlowEdgeKind::Data
            && let Some(name) = edge.to_pin.strip_prefix("use:")
            && let Some(initial) = definitions.get(&(edge.from.as_str(), edge.from_pin.as_str()))
        {
            reads
                .entry(edge.to.clone())
                .or_default()
                .insert(name.into(), initial.cloned());
        }
    }
    reads
}

pub(crate) fn annotate(flow: &mut FlowGraph, facts: &TypeFactSites, locals: &[&LocalTypeFact]) {
    fn apply(expr: &mut Expr, facts: &TypeFactSites, dynamic: Option<&ScriptReads>) {
        // The TS checker can hold an untyped JS binding to its initializer's type,
        // even when a reaching assignment changed the value's category.
        let changed = expr.concept == Concept::Var && has_reaching_type_change(expr, dynamic);
        if let Some(info) = facts.get(&(
            expr.span.start_byte,
            expr.span.end_byte,
            ValueFactTarget::Expression,
        )) && (!changed
            || expr
                .type_info
                .as_ref()
                .is_some_and(|syntax| syntax.shape == info.shape))
        {
            expr.type_info = Some(info.clone());
        }
        for pin in expr.param_pins.iter_mut().flatten() {
            if let Some(span) = &pin.declaration
                && let Some(info) =
                    facts.get(&(span.start_byte, span.end_byte, ValueFactTarget::Binding))
            {
                pin.type_info = Some(info.clone());
            }
        }
        let child_dynamic = if expr.concept == Concept::Lambda {
            None
        } else {
            dynamic
        };
        for child in &mut expr.children {
            apply(child, facts, child_dynamic);
        }
    }
    fn has_reaching_type_change(expr: &Expr, dynamic: Option<&ScriptReads>) -> bool {
        let Some(initial) = dynamic.and_then(|reads| reads.get(&expr.label)) else {
            return false;
        };
        let Some(info) = expr
            .type_info
            .as_ref()
            .filter(|info| info.origin == TypeOrigin::Syntax)
        else {
            return false;
        };
        if info.shape == TypeShape::Unknown {
            return initial
                .as_ref()
                .is_some_and(|shape| *shape != TypeShape::Unknown);
        }
        initial.as_ref() != Some(&info.shape)
    }
    let dynamic = dynamic_script_reads(flow, facts);
    let rust_locals: HashMap<_, _> = locals
        .iter()
        .filter(|local| flow.language == Language::Rust && local.site.file == flow.span.file)
        .map(|local| ((local.site.start, local.site.end), local.ty.as_str()))
        .collect();
    for node in &mut flow.nodes {
        if let Some(expr) = &mut node.expr {
            apply(expr, facts, dynamic.get(&node.id));
        }
        for pin in &mut node.outputs {
            let Some(span) = &pin.declaration else {
                continue;
            };
            let info = facts
                .get(&(span.start_byte, span.end_byte, ValueFactTarget::Binding))
                .cloned()
                .or_else(|| {
                    let ty = rust_locals.get(&(span.start_byte, span.end_byte))?;
                    Some(TypeInfo::new(
                        native_shape(Language::Rust, ty),
                        TypeOrigin::Compiler,
                        Some((*ty).to_owned()),
                    ))
                });
            if let Some(info) = info {
                pin.type_info = Some(info);
            }
        }
    }
    nodex_lang_api::blueprint_types::carry_reads(flow);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    Front,
    Back,
}

enum Usage {
    Insert(End),
    Remove(End),
    Observe,
}

#[derive(Default)]
struct Evidence {
    insert: Option<(End, Span)>,
    remove: Option<(End, Span)>,
    escaped: bool,
}

impl Evidence {
    fn record(&mut self, usage: Usage, span: &Span) {
        let (end, slot) = match usage {
            Usage::Insert(end) => (end, &mut self.insert),
            Usage::Remove(end) => (end, &mut self.remove),
            Usage::Observe => return,
        };
        if slot.as_ref().is_some_and(|(previous, _)| *previous != end) {
            self.escaped = true;
        } else {
            slot.get_or_insert_with(|| (end, span.clone()));
        }
    }

    fn role(self) -> Option<RoleEvidence> {
        if self.escaped {
            return None;
        }
        let (insert_end, insert) = self.insert?;
        let (remove_end, remove) = self.remove?;
        let role = if insert_end == remove_end {
            CollectionRole::Stack
        } else {
            CollectionRole::Queue
        };
        Some(RoleEvidence {
            role,
            insert,
            remove,
        })
    }
}

fn usage(language: Language, family: CollectionKind, reference: &SemanticRef) -> Option<Usage> {
    use CollectionKind::{Array, Deque, List};
    use End::{Back, Front};
    use Usage::{Insert, Remove};
    let target = &reference.target;
    if !target.external || target.kind == "unresolved" {
        return None;
    }
    let head = target.display.split('(').next()?;
    let member = head.rsplit(['.', ':']).next()?;
    Some(
        match (language, family, target.container.as_deref(), member) {
            (Language::Typescript, Array, Some("Array"), "push") if target.linq => Insert(Back),
            (Language::Typescript, Array, Some("Array"), "unshift") if target.linq => Insert(Front),
            (Language::Typescript, Array, Some("Array"), "pop") if target.linq => Remove(Back),
            (Language::Typescript, Array, Some("Array"), "shift") if target.linq => Remove(Front),
            (Language::Rust, List, _, "push") if head.starts_with("alloc::vec::Vec::") => {
                Insert(Back)
            }
            (Language::Rust, List, _, "pop") if head.starts_with("alloc::vec::Vec::") => {
                Remove(Back)
            }
            (Language::Rust, List, _, "len" | "is_empty")
                if head.starts_with("alloc::vec::Vec::") =>
            {
                Usage::Observe
            }
            (Language::Rust, Deque, _, "push_back")
                if head.starts_with("alloc::collections::vec_deque::VecDeque::") =>
            {
                Insert(Back)
            }
            (Language::Rust, Deque, _, "push_front")
                if head.starts_with("alloc::collections::vec_deque::VecDeque::") =>
            {
                Insert(Front)
            }
            (Language::Rust, Deque, _, "pop_back")
                if head.starts_with("alloc::collections::vec_deque::VecDeque::") =>
            {
                Remove(Back)
            }
            (Language::Rust, Deque, _, "pop_front")
                if head.starts_with("alloc::collections::vec_deque::VecDeque::") =>
            {
                Remove(Front)
            }
            (Language::Rust, Deque, _, "len" | "is_empty" | "front" | "back")
                if head.starts_with("alloc::collections::vec_deque::VecDeque::") =>
            {
                Usage::Observe
            }
            (Language::Csharp, List, Some(container), "AddLast")
                if container.starts_with("System.Collections.Generic.LinkedList<") =>
            {
                Insert(Back)
            }
            (Language::Csharp, List, Some(container), "AddFirst")
                if container.starts_with("System.Collections.Generic.LinkedList<") =>
            {
                Insert(Front)
            }
            (Language::Csharp, List, Some(container), "RemoveLast")
                if container.starts_with("System.Collections.Generic.LinkedList<") =>
            {
                Remove(Back)
            }
            (Language::Csharp, List, Some(container), "RemoveFirst")
                if container.starts_with("System.Collections.Generic.LinkedList<") =>
            {
                Remove(Front)
            }
            _ => return None,
        },
    )
}

fn inspect_uses(
    expr: &Expr,
    is_binding: &impl Fn(&Expr) -> bool,
    resolve: &impl Fn(&Expr) -> Option<Usage>,
    array_length: bool,
    writing: bool,
    accepted: &mut impl FnMut(&Span),
    evidence: &mut Evidence,
) -> bool {
    if is_binding(expr) {
        evidence.escaped = true;
        return true;
    }
    let mut found = false;
    let safe_length = array_length
        && !writing
        && matches!(
            expr.concept,
            Concept::GetMember | Concept::SafeAccess | Concept::Operation
        )
        && matches!(expr.label.as_str(), ".length" | "?.length")
        && expr
            .type_info
            .as_ref()
            .is_some_and(|info| info.shape == TypeShape::scalar("Number"));
    let writes_operand =
        expr.concept == Concept::Assign || matches!(expr.label.as_str(), "++" | "--" | "delete");
    for (index, child) in expr.children.iter().enumerate() {
        if index == 0
            && is_binding(child)
            && let Some(usage) = if safe_length {
                Some(Usage::Observe)
            } else if child.flavors.iter().any(|flavor| flavor == "target") {
                resolve(expr)
            } else {
                None
            }
        {
            evidence.record(usage, &expr.span);
            accepted(&child.span);
            found = true;
        } else {
            found |= inspect_uses(
                child,
                is_binding,
                resolve,
                array_length,
                writes_operand && index == 0,
                accepted,
                evidence,
            );
        }
    }
    found
}

pub(crate) fn roles<'a>(
    flow: &mut FlowGraph,
    refs: &[&SemanticRef],
    binding_uses: impl Iterator<Item = &'a BindingUseFact>,
) {
    let mut calls = HashMap::new();
    for &reference in refs
        .iter()
        .filter(|r| r.kind == RefKind::Call && r.site.file == flow.span.file)
    {
        calls
            .entry((reference.site.start, reference.site.end))
            .and_modify(|entry| *entry = None)
            .or_insert(Some(reference));
    }
    let positions: HashMap<_, _> = flow
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();
    let typescript = flow.language == Language::Typescript;
    let exact_bindings = typescript || flow.language == Language::Csharp;
    let mut candidates = HashMap::new();
    let mut binding_sites = HashMap::new();
    for (i, node) in flow.nodes.iter().enumerate() {
        for (j, pin) in node.outputs.iter().enumerate() {
            if let Some(TypeShape::Collection {
                family:
                    family @ (CollectionKind::Array | CollectionKind::List | CollectionKind::Deque),
                ..
            }) = pin.type_info.as_ref().map(|info| &info.shape)
            {
                let key = (node.id.as_str(), pin.id.as_str());
                candidates.insert(key, (i, j, *family));
                if exact_bindings && let Some(declaration) = &pin.declaration {
                    binding_sites.insert((declaration.start_byte, declaration.end_byte), key);
                }
            }
        }
    }
    let binding_uses: Vec<_> = binding_uses
        .filter(|fact| fact.declaration.file == flow.span.file)
        .filter_map(|fact| {
            let key = binding_sites.get(&(fact.declaration.start, fact.declaration.end))?;
            Some((*key, &fact.site))
        })
        .collect();
    let resolved: HashSet<_> = binding_uses
        .iter()
        .filter(|(_, site)| site.file == flow.span.file)
        .map(|(key, site)| {
            let (node, pin, _) = candidates[key];
            ((node, pin), site.start, site.end)
        })
        .collect();
    let exact_use = |binding, expr: &Expr| {
        resolved.contains(&(binding, expr.span.start_byte, expr.span.end_byte))
    };
    // A direct immutable JavaScript alias refers to the same collection. Any other transfer is
    // treated as an escape, including passing a tracked alias to a call that can keep it.
    let mut aliases = HashMap::new();
    let mut alias_inputs = HashMap::new();
    if typescript {
        for edge in flow
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Data)
        {
            let source = (edge.from.as_str(), edge.from_pin.as_str());
            let Some(&(from, output, family)) = candidates.get(&source) else {
                continue;
            };
            let Some(&i) = positions.get(edge.to.as_str()) else {
                continue;
            };
            let node = &flow.nodes[i];
            let mut bindings = node.outputs.iter().filter(|pin| pin.id.starts_with("var:"));
            let Some(pin) = bindings.next().filter(|_| bindings.next().is_none()) else {
                continue;
            };
            let alias = (node.id.as_str(), pin.id.as_str());
            if from < i
                && pin
                    .binding
                    .as_ref()
                    .is_some_and(|rule| rule.form == BindingForm::Const)
                && node.concept == Concept::Var
                && node.expr.as_ref().is_some_and(|expr| {
                    expr.concept == Concept::Var
                        && edge.to_pin.strip_prefix("use:") == Some(expr.label.as_str())
                        && exact_use((from, output), expr)
                })
                && candidates
                    .get(&alias)
                    .is_some_and(|&(_, _, alias_family)| alias_family == family)
            {
                aliases.insert(alias, source);
                alias_inputs.insert(alias.0, source);
            }
        }
    }
    let root = |mut key| {
        // A direct alias has to follow the existing lexical definition.
        while let Some(&source) = aliases.get(&key) {
            key = source;
        }
        key
    };
    let mut usage_by_root: HashMap<_, Evidence> = HashMap::new();
    for node in &flow.nodes {
        for write in &node.binding_writes {
            let key = (write.node.as_str(), write.pin.as_str());
            if candidates.contains_key(&key) {
                usage_by_root.entry(root(key)).or_default().escaped = true;
            }
        }
    }
    let mut seen_uses = HashSet::new();
    for edge in flow
        .edges
        .iter()
        .filter(|edge| edge.kind == FlowEdgeKind::Data)
    {
        let key = (edge.from.as_str(), edge.from_pin.as_str());
        let Some(&(from, output, family)) = candidates.get(&key) else {
            continue;
        };
        let Some(name) = edge.to_pin.strip_prefix("use:") else {
            continue;
        };
        let evidence = usage_by_root.entry(root(key)).or_default();
        let Some(expr) = positions
            .get(edge.to.as_str())
            .and_then(|&i| flow.nodes[i].expr.as_ref())
        else {
            evidence.escaped = true;
            continue;
        };
        let mut accepted = |span: &Span| {
            seen_uses.insert((key, span.start_byte, span.end_byte));
        };
        if alias_inputs.get(edge.to.as_str()) == Some(&key) {
            accepted(&expr.span);
            continue;
        }
        let resolve = |expr: &Expr| {
            let reference = calls
                .get(&(expr.span.start_byte, expr.span.end_byte))
                .copied()
                .flatten()?;
            usage(flow.language, family, reference)
        };
        let is_binding = |expr: &Expr| {
            expr.concept == Concept::Var
                && expr.label == name
                && (!exact_bindings || exact_use((from, output), expr))
        };
        if !inspect_uses(
            expr,
            &is_binding,
            &resolve,
            typescript && family == CollectionKind::Array,
            false,
            &mut accepted,
            evidence,
        ) {
            evidence.escaped = true;
        }
    }
    // Compiler-resolved local uses also cover code the visual graph folds away, including
    // loop updates and references captured by nested functions.
    for (key, site) in binding_uses {
        if site.file != flow.span.file || !seen_uses.contains(&(key, site.start, site.end)) {
            usage_by_root.entry(root(key)).or_default().escaped = true;
        }
    }
    let confirmed: HashMap<_, _> = usage_by_root
        .into_iter()
        .filter_map(|(key, evidence)| Some((key, evidence.role()?)))
        .collect();
    let roles: Vec<_> = candidates
        .iter()
        .filter_map(|(&key, &(i, j, _))| confirmed.get(&root(key)).map(|role| (i, j, role.clone())))
        .collect();
    let reads: Vec<_> = flow
        .edges
        .iter()
        .filter(|edge| edge.kind == FlowEdgeKind::Data)
        .filter_map(|edge| {
            let key = (edge.from.as_str(), edge.from_pin.as_str());
            let &(from, output, family) = candidates.get(&key)?;
            let role = confirmed.get(&root(key))?;
            let &i = positions.get(edge.to.as_str())?;
            Some((
                i,
                (from, output),
                edge.to_pin.strip_prefix("use:")?.to_owned(),
                family,
                role.clone(),
            ))
        })
        .collect();
    for (i, j, role) in roles {
        if let Some(info) = &mut flow.nodes[i].outputs[j].type_info {
            info.role = Some(role);
        }
    }
    for (i, binding, name, family, role) in reads {
        let node = &mut flow.nodes[i];
        if let Some(pin) = node
            .inputs
            .iter_mut()
            .find(|pin| pin.id == format!("use:{name}"))
        {
            attach_role(&mut pin.type_info, family, &role);
        }
        if let Some(expr) = &mut node.expr {
            let is_binding = |expr: &Expr| {
                expr.concept == Concept::Var
                    && expr.label == name
                    && (!exact_bindings || exact_use(binding, expr))
            };
            mark_role_read(expr, &is_binding, family, &role);
        }
    }
}

fn attach_role(info: &mut Option<TypeInfo>, family: CollectionKind, role: &RoleEvidence) {
    if let Some(info) = info
        && matches!(&info.shape, TypeShape::Collection { family: used, .. } if *used == family)
    {
        info.role = Some(role.clone());
    }
}

fn mark_role_read(
    expr: &mut Expr,
    is_binding: &impl Fn(&Expr) -> bool,
    family: CollectionKind,
    role: &RoleEvidence,
) {
    if expr.concept == Concept::Lambda {
        return;
    }
    if is_binding(expr) {
        attach_role(&mut expr.type_info, family, role);
    }
    for child in &mut expr.children {
        mark_role_read(child, is_binding, family, role);
    }
}
