//! Class diagrams: the types of a scope with their members, stereotypes and relations.
//!
//! Relations are lifted from members to their types and merged per pair of types; the strongest
//! wins (inheritance > realization > composition > aggregation > association > dependency):
//! - inheritance / realization from `Inherits` / `Implements` (and DI registrations without a
//!   declared interface);
//! - associations from fields and properties, with the multiplicity their type shows
//!   (`List<T>` → `0..*`, `T?` → `0..1`); composition when the field holds the part by value
//!   (Rust fields, `Box<T>`, value types, `readonly` fields built with `new`), aggregation when it
//!   is shared (`Rc`/`Arc`) or injected through the constructor;
//! - associations between data models from their foreign keys and navigation properties;
//! - dependencies from parameters, return types, calls, `new`, events and data writes.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use nodex_ir::knowledge::Confidence;
use nodex_ir::{Language, StructEdgeKind, Symbol, SymbolKind};

use crate::index::{Index, package_key};
use crate::mermaid::{Ids, class_defs, class_text, label};
use crate::scope::Scoped;
use crate::types::{Holding, analyze, uml_parameters};
use crate::{MemberFilter, UmlDiagram, UmlElement, UmlKind, UmlOptions, UmlRelation, empty};

const MAX_TYPES: usize = 150;
const MAX_NEIGHBORS: usize = 40;
const MAX_MEMBERS: usize = 14;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RelKind {
    Dependency,
    Association,
    Aggregation,
    Composition,
    Realization,
    Inheritance,
}

impl RelKind {
    fn name(self) -> &'static str {
        match self {
            RelKind::Dependency => "dependency",
            RelKind::Association => "association",
            RelKind::Aggregation => "aggregation",
            RelKind::Composition => "composition",
            RelKind::Realization => "realizes",
            RelKind::Inheritance => "inherits",
        }
    }
}

/// A relation between two types: `from` is the child, the whole, the user; `to` the parent,
/// the part, the used type.
#[derive(Clone, Debug)]
struct Rel {
    kind: RelKind,
    label: Option<String>,
    /// Multiplicity at `from` and at `to`.
    mult: (Option<String>, Option<String>),
    confidence: Option<Confidence>,
    evidence: String,
    site: Option<nodex_ir::Span>,
    count: u32,
}

pub fn build(index: &Index, scoped: &Scoped, opts: &UmlOptions) -> UmlDiagram {
    let mut out = empty(UmlKind::Class);
    let hide = |id: &str| opts.hide_tests && index.is_test(id);
    let in_scope: BTreeSet<&str> = scoped.types.iter().map(String::as_str).filter(|t| !hide(t)).filter(|t| shown_kind(index, t)).collect();
    if in_scope.is_empty() {
        out.warnings.push("No classes, interfaces or other types in this scope.".into());
        out.mermaid = "classDiagram\n  note \"No types in this scope\"\n".into();
        return out;
    }

    let rels = relations(index, opts);
    // Neighbors: types just outside the scope that a scope type relates to.
    let mut neighbors: BTreeMap<&str, u32> = BTreeMap::new();
    if opts.context_neighbors {
        for (a, b) in rels.keys() {
            let (a, b) = (a.as_str(), b.as_str());
            for (inside, other) in [(a, b), (b, a)] {
                if in_scope.contains(inside) && !in_scope.contains(other) && !hide(other) && shown_kind(index, other) {
                    let external = index.get(other).is_none_or(|s| s.external);
                    if !external || opts.include_external {
                        *neighbors.entry(other).or_default() += 1;
                    }
                }
            }
        }
    }
    let mut neighbor_list: Vec<(&str, u32)> = neighbors.into_iter().collect();
    neighbor_list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    if neighbor_list.len() > MAX_NEIGHBORS {
        out.warnings.push(format!("{} related types outside the scope are not shown.", neighbor_list.len() - MAX_NEIGHBORS));
        out.stats.hidden += (neighbor_list.len() - MAX_NEIGHBORS) as u32;
        neighbor_list.truncate(MAX_NEIGHBORS);
    }

    // The most connected types first when there are too many.
    let limit = opts.limit(MAX_TYPES);
    let mut degree: HashMap<&str, u32> = HashMap::new();
    for ((a, b), r) in &rels {
        *degree.entry(a.as_str()).or_default() += r.count;
        *degree.entry(b.as_str()).or_default() += r.count;
    }
    let mut ranked: Vec<&str> = in_scope.iter().copied().collect();
    ranked.sort_by(|a, b| {
        let score = |t: &str| degree.get(t).copied().unwrap_or(0) + if index.routed.contains_key(t) { 5 } else { 0 };
        score(b).cmp(&score(a)).then(a.cmp(b))
    });
    let mut hidden_types = 0;
    if ranked.len() > limit {
        hidden_types = ranked.len() - limit;
        ranked.truncate(limit);
        out.truncated = true;
        out.warnings.push(format!("{hidden_types} less connected types are not shown; pick a folder or a namespace to see them."));
        out.stats.hidden += hidden_types as u32;
    }
    ranked.sort();
    let shown: BTreeSet<&str> = ranked.iter().copied().chain(neighbor_list.iter().map(|(t, _)| *t)).collect();
    let context: BTreeSet<&str> = neighbor_list.iter().map(|(t, _)| *t).collect();

    let members = match opts.members {
        MemberFilter::Auto if scoped.broad => MemberFilter::Public,
        MemberFilter::Auto => MemberFilter::All,
        m => m,
    };

    // Ids in a stable order: namespaces, then types by id.
    let mut ids = Ids::new("n");
    let mut by_ns: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for t in &shown {
        let ns = index.get(t).map(|s| package_key(index, s).0).unwrap_or_default();
        by_ns.entry(ns).or_default().push(t);
    }
    let grouped = by_ns.len() > 1;
    let mut text = String::from("classDiagram\n  direction TB\n");
    for (ns, types) in &by_ns {
        let indent = if grouped && !ns.is_empty() {
            text.push_str(&format!("  namespace {} {{\n", namespace_name(ns)));
            "    "
        } else {
            "  "
        };
        for t in types {
            let id = ids.id(t);
            let sym = index.get(t);
            let name = sym.map(|s| s.name.clone()).unwrap_or_else(|| t.trim_start_matches("ext:").to_owned());
            let is_context = context.contains(t);
            let stereotype = sym.and_then(|s| stereotype(index, s));
            let body = if is_context { Vec::new() } else { sym.map(|s| member_lines(index, s, members)).unwrap_or_default() };
            text.push_str(&format!("{indent}class {id}[\"{}\"]", label(&name, 60)));
            if stereotype.is_some() || !body.is_empty() {
                text.push_str(" {\n");
                if let Some(st) = &stereotype {
                    text.push_str(&format!("{indent}  <<{st}>>\n"));
                }
                for line in &body {
                    text.push_str(&format!("{indent}  {line}\n"));
                }
                text.push_str(&format!("{indent}}}\n"));
            } else {
                text.push('\n');
            }
            out.elements.push(UmlElement {
                mermaid_id: id,
                symbol: sym.map(|s| s.id.clone()),
                label: name,
                kind: sym.map(element_kind).unwrap_or("class").to_owned(),
                span: sym.and_then(|s| s.origins.first().cloned()),
                context: is_context,
                detail: stereotype.map(|s| format!("«{s}»")),
            });
        }
        if grouped && !ns.is_empty() {
            text.push_str("  }\n");
        }
    }

    for ((a, b), r) in &rels {
        let (Some(from), Some(to)) = (ids.get(a), ids.get(b)) else { continue };
        if !shown.contains(a.as_str()) || !shown.contains(b.as_str()) {
            continue;
        }
        if context.contains(a.as_str()) && context.contains(b.as_str()) {
            continue;
        }
        let m = |x: &Option<String>| x.as_ref().map(|v| format!(" \"{v}\"")).unwrap_or_default();
        let arrow = match r.kind {
            RelKind::Inheritance => format!("{to} <|-- {from}"),
            RelKind::Realization => format!("{to} <|.. {from}"),
            RelKind::Composition => format!("{from}{} *--{} {to}", m(&r.mult.0), m(&r.mult.1)),
            RelKind::Aggregation => format!("{from}{} o--{} {to}", m(&r.mult.0), m(&r.mult.1)),
            RelKind::Association => format!("{from}{} -->{} {to}", m(&r.mult.0), m(&r.mult.1)),
            RelKind::Dependency => format!("{from} ..> {to}"),
        };
        let mut lbl = r.label.clone().unwrap_or_default();
        if r.confidence.is_some_and(|c| c != Confidence::Exact) {
            lbl = if lbl.is_empty() { "inferred".into() } else { format!("{lbl} (inferred)") };
        }
        let lbl = class_text(&lbl, 40);
        if lbl.is_empty() {
            text.push_str(&format!("  {arrow}\n"));
        } else {
            text.push_str(&format!("  {arrow} : {lbl}\n"));
        }
        out.relations.push(UmlRelation {
            from: from.to_owned(),
            to: to.to_owned(),
            kind: r.kind.name().into(),
            label: r.label.clone(),
            confidence: r.confidence,
            evidence: Some(r.evidence.clone()),
            site: r.site.clone(),
            count: r.count,
        });
    }

    if hidden_types > 0 {
        text.push_str(&format!("  note \"+{hidden_types} more types not shown\"\n"));
    }
    let ctx_ids: Vec<String> = out.elements.iter().filter(|e| e.context).map(|e| e.mermaid_id.clone()).collect();
    let ext_ids: Vec<String> = out.elements.iter().filter(|e| e.symbol.as_deref().is_some_and(|s| index.get(s).is_some_and(|x| x.external))).map(|e| e.mermaid_id.clone()).collect();
    let mut used = Vec::new();
    if !ctx_ids.is_empty() {
        used.push("ctx");
    }
    if !ext_ids.is_empty() {
        used.push("ext");
    }
    class_defs(&mut text, &used);
    for id in ctx_ids {
        text.push_str(&format!("  class {id}:::ctx\n"));
    }
    for id in ext_ids {
        text.push_str(&format!("  class {id}:::ext\n"));
    }
    out.mermaid = text;
    out
}

/// Types a class diagram draws (not delegates of callbacks or type-only unions of literals).
fn shown_kind(index: &Index, id: &str) -> bool {
    match index.get(id) {
        Some(s) => s.kind.is_type(),
        None => id.starts_with("ext:"),
    }
}

/// Mermaid namespaces are words: `Sample.Domain` stays, `src/domain` reads `src.domain`.
fn namespace_name(ns: &str) -> String {
    let cleaned: String = ns.replace("::", ".").replace(['/', '-', ' '], ".").chars().filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_').collect();
    let cleaned = cleaned.trim_matches('.').to_owned();
    if cleaned.is_empty() || cleaned.chars().next().is_some_and(|c| c.is_ascii_digit()) { format!("ns_{cleaned}") } else { cleaned }
}

fn element_kind(s: &Symbol) -> &'static str {
    match s.kind {
        SymbolKind::Interface => "interface",
        SymbolKind::Trait => "trait",
        SymbolKind::Enum => "enum",
        SymbolKind::Struct => "struct",
        SymbolKind::Record => "record",
        SymbolKind::Union => "union",
        _ => "class",
    }
}

/// One stereotype per class: what it is to the application when the knowledge graph says so
/// (an entity, a controller, a registered service), else what kind of type it is.
fn stereotype(index: &Index, s: &Symbol) -> Option<String> {
    if s.external {
        return Some("external".into());
    }
    if index.models.contains_key(s.id.as_str()) {
        let table = index.models.get(s.id.as_str()).and_then(|m| m.table.clone());
        return Some(match table {
            Some(t) => format!("entity {t}"),
            None => "entity".into(),
        });
    }
    if index.routed.contains_key(s.id.as_str()) {
        return Some("controller".into());
    }
    if index.is_test(&s.id) {
        return Some("test".into());
    }
    let has = |f: &str| s.flavors.iter().any(|x| x == f);
    match s.kind {
        SymbolKind::Interface => return Some("interface".into()),
        SymbolKind::Trait => return Some("trait".into()),
        SymbolKind::Enum => return Some("enumeration".into()),
        SymbolKind::Union => return Some("union".into()),
        SymbolKind::Record => return Some("record".into()),
        SymbolKind::Delegate => return Some("delegate".into()),
        SymbolKind::TypeAlias => return Some("type".into()),
        _ => {}
    }
    if let Some(lifetime) = index.di_lifetime(&s.id) {
        return Some(format!("service {lifetime}"));
    }
    if has("unity:MonoBehaviour") || s.flavors.iter().any(|f| f.starts_with("unity:")) && s.kind == SymbolKind::Class {
        return Some("MonoBehaviour".into());
    }
    if s.flavors.iter().any(|f| f == "react:component" || f == "angular:component" || f == "vue:component") {
        return Some("component".into());
    }
    if has("abstract") {
        return Some("abstract".into());
    }
    if has("static") && s.kind == SymbolKind::Class {
        return Some("static".into());
    }
    if s.kind == SymbolKind::Struct {
        return Some("struct".into());
    }
    None
}

fn visibility(s: &Symbol, parent_kind: SymbolKind) -> Option<char> {
    let has = |f: &str| s.flavors.iter().any(|x| x == f);
    if has("private") || s.name.starts_with('#') {
        return Some('-');
    }
    if has("protected") {
        return Some('#');
    }
    if has("internal") || s.flavors.iter().any(|f| f.starts_with("pub(")) {
        return Some('~');
    }
    if has("public") || has("pub") || has("export") {
        return Some('+');
    }
    // Defaults: interface and trait members are public; TypeScript members are public; C# and
    // Rust members are private.
    if matches!(parent_kind, SymbolKind::Interface | SymbolKind::Trait | SymbolKind::Enum | SymbolKind::Union) {
        return Some('+');
    }
    match s.language {
        Language::Typescript => Some('+'),
        Language::Csharp | Language::Rust => {
            if s.flavors.iter().any(|f| f.starts_with("from-trait:") || f == "positional" || f == "override") {
                Some('+')
            } else {
                Some('-')
            }
        }
    }
}

/// Lines of a class body: attributes, then operations; enum members for enumerations.
fn member_lines(index: &Index, s: &Symbol, filter: MemberFilter) -> Vec<String> {
    let Some(kids) = index.children.get(s.id.as_str()) else { return Vec::new() };
    if filter == MemberFilter::None && !matches!(s.kind, SymbolKind::Enum) {
        return Vec::new();
    }
    let csharp = s.language == Language::Csharp;
    let mut attrs: Vec<(u8, String)> = Vec::new();
    let mut ops: Vec<(u8, String)> = Vec::new();
    for m in kids {
        let skip = m.flavors.iter().any(|f| matches!(f.as_str(), "callback" | "local-function" | "getter" | "setter"));
        if skip {
            continue;
        }
        let vis = visibility(m, s.kind);
        if filter == MemberFilter::Public && !matches!(vis, Some('+') | None) && m.kind != SymbolKind::EnumMember {
            continue;
        }
        let v = vis.map(String::from).unwrap_or_default();
        let is_static = m.flavors.iter().any(|f| f == "static");
        let is_abstract = m.flavors.iter().any(|f| f == "abstract" || f == "required") && s.kind != SymbolKind::Interface && s.kind != SymbolKind::Trait;
        let name = m.name.trim_start_matches('#').split('~').next().unwrap_or(&m.name).to_owned();
        match m.kind {
            SymbolKind::EnumMember => {
                let shown = m.signature.as_deref().filter(|sig| sig.contains('(') || sig.contains('{')).unwrap_or(&name);
                attrs.push((0, class_text(shown, 60)));
            }
            SymbolKind::Field | SymbolKind::Property | SymbolKind::Event => {
                let ty = m.type_text.as_deref().map(|t| format!(": {t}")).unwrap_or_default();
                let st = if is_static { "$" } else { "" };
                let rank = if vis == Some('+') { 0 } else { 1 };
                attrs.push((rank, class_text(&format!("{v}{name}{ty}{st}"), 70)));
            }
            SymbolKind::Method | SymbolKind::Function => {
                let params = m.signature.as_deref().map(|sig| uml_parameters(sig, &m.name, csharp)).unwrap_or_default();
                let ret = m.type_text.as_deref().filter(|t| !matches!(*t, "void" | "()" | "Self")).map(|t| format!(" {t}")).unwrap_or_default();
                let marks = format!("{}{}", if is_abstract { "*" } else { "" }, if is_static { "$" } else { "" });
                let rank = if vis == Some('+') { 0 } else { 1 };
                ops.push((rank, class_text(&format!("{v}{name}({params}){marks}{ret}"), 90)));
            }
            _ => {}
        }
    }
    attrs.sort_by_key(|a| a.0);
    ops.sort_by_key(|a| a.0);
    let mut lines: Vec<String> = attrs.into_iter().map(|(_, l)| l).chain(ops.into_iter().map(|(_, l)| l)).filter(|l| !l.is_empty()).collect();
    if lines.len() > MAX_MEMBERS {
        let more = lines.len() - (MAX_MEMBERS - 1);
        lines.truncate(MAX_MEMBERS - 1);
        lines.push(format!("…+{more} more"));
    }
    lines
}

/// Every type-level relation of the project, merged per pair of types.
fn relations(index: &Index, opts: &UmlOptions) -> BTreeMap<(String, String), Rel> {
    let mut rels: BTreeMap<(String, String), Rel> = BTreeMap::new();
    let mut add = |a: &str, b: &str, rel: Rel| {
        if a == b && rel.kind != RelKind::Association && rel.kind != RelKind::Composition && rel.kind != RelKind::Aggregation {
            return;
        }
        let key = (a.to_owned(), b.to_owned());
        match rels.get_mut(&key) {
            Some(old) => {
                old.count += rel.count;
                // The strongest kind wins; for the same kind, the one that says more (a label, both multiplicities).
                let says_more = (old.label.is_none() && rel.label.is_some()) || (old.mult.0.is_none() && rel.mult.0.is_some());
                if rel.kind > old.kind || (rel.kind == old.kind && says_more) {
                    let count = old.count;
                    *old = rel;
                    old.count = count;
                }
            }
            None => {
                rels.insert(key, rel);
            }
        }
    };
    let rel = |kind: RelKind, label: Option<String>, evidence: String| Rel {
        kind,
        label,
        mult: (None, None),
        confidence: None,
        evidence,
        site: None,
        count: 1,
    };
    // Constructor injection: the consumer aggregates the service.
    let injected: BTreeSet<(String, String)> = index
        .graph
        .edges
        .iter()
        .filter(|e| e.kind == StructEdgeKind::DependsOn)
        .map(|e| (e.from.clone(), e.to.clone()))
        .collect();
    let declared: BTreeSet<(String, String)> = index
        .graph
        .edges
        .iter()
        .filter(|e| matches!(e.kind, StructEdgeKind::Implements | StructEdgeKind::Inherits))
        .map(|e| (e.from.clone(), e.to.clone()))
        .collect();

    for e in index.relations() {
        if !opts.allows(e.confidence) {
            continue;
        }
        let (Some(from), Some(to)) = (lift(index, &e.from), lift(index, &e.to)) else { continue };
        let name = |id: &str| index.get(id).map(|s| s.name.clone()).unwrap_or_else(|| id.trim_start_matches("ext:").to_owned());
        let member = index.get(&e.from);
        let mut r = match e.kind {
            StructEdgeKind::Inherits => rel(RelKind::Inheritance, None, format!("{} extends {}", name(&from), name(&to))),
            StructEdgeKind::Implements => rel(RelKind::Realization, None, format!("{} implements {}", name(&from), name(&to))),
            // `IOrderService` → `OrderService`: the implementation realizes the service.
            StructEdgeKind::ResolvesDi => {
                if declared.contains(&(to.clone(), from.clone())) {
                    continue;
                }
                let mut r = rel(RelKind::Realization, Some("DI".into()), e.evidence.clone().unwrap_or_default());
                r.site = e.site.clone();
                add(&to, &from, r);
                continue;
            }
            StructEdgeKind::DependsOn => {
                let mut r = rel(RelKind::Aggregation, Some("injected".into()), e.evidence.clone().unwrap_or_default());
                r.mult = (None, Some("1".into()));
                r
            }
            StructEdgeKind::References => match member {
                Some(m) if matches!(m.kind, SymbolKind::Field | SymbolKind::Property | SymbolKind::Event) && m.parent.as_deref() == Some(from.as_str()) => {
                    structural(index, m, &from, &to, &injected)
                }
                _ => rel(RelKind::Dependency, None, format!("{} uses {}", name(&e.from), name(&to))),
            },
            StructEdgeKind::Instantiates => rel(RelKind::Dependency, Some("create".into()), format!("{} creates {}", name(&e.from), name(&to))),
            StructEdgeKind::Calls => rel(RelKind::Dependency, None, format!("{} calls {}", name(&e.from), name(&e.to))),
            StructEdgeKind::Raises | StructEdgeKind::Subscribes => rel(RelKind::Dependency, Some("event".into()), format!("{} {} {}", name(&e.from), if e.kind == StructEdgeKind::Raises { "raises" } else { "handles" }, name(&e.to))),
            StructEdgeKind::Mutates => rel(RelKind::Dependency, Some("writes".into()), e.evidence.clone().unwrap_or_else(|| format!("{} changes {}", name(&e.from), name(&to)))),
            StructEdgeKind::Compatible => rel(RelKind::Realization, Some("fits".into()), format!("{} fits {} structurally", name(&from), name(&to))),
            _ => continue,
        };
        if r.confidence.is_none() {
            r.confidence = e.confidence;
        }
        if r.site.is_none() {
            r.site = e.site.clone().or_else(|| member.and_then(|m| m.origins.first().cloned()));
        }
        add(&from, &to, r);
    }

    // Fields and properties, read from their declared type (some languages report the type
    // reference on the owner, not on the member).
    let mut by_name: HashMap<&str, Vec<&Symbol>> = HashMap::new();
    for s in index.graph.symbols.iter().filter(|s| s.kind.is_type()) {
        by_name.entry(s.name.as_str()).or_default().push(s);
    }
    for m in index.graph.symbols.iter().filter(|s| matches!(s.kind, SymbolKind::Field | SymbolKind::Property) && !s.external) {
        let Some(owner) = m.parent.as_deref().and_then(|p| index.get(p)).filter(|p| p.kind.is_type()) else { continue };
        let Some(shape) = m.type_text.as_deref().and_then(analyze) else { continue };
        let Some(candidates) = by_name.get(shape.target.as_str()) else { continue };
        let same_lang: Vec<&&Symbol> = candidates.iter().filter(|c| c.language == owner.language && !c.external).collect();
        let ns = index.namespace_of(&owner.id).map(|n| n.id.clone());
        let Some(target) = same_lang
            .iter()
            .find(|c| index.namespace_of(&c.id).map(|n| n.id.clone()) == ns)
            .or(same_lang.first())
            .map(|c| c.id.clone())
        else {
            continue;
        };
        add(&owner.id, &target, structural(index, m, &owner.id, &target, &injected));
    }

    // Data models: foreign keys and navigation properties.
    for m in &index.knowledge.models {
        let Some(owner) = index.get(&m.symbol) else { continue };
        for f in &m.model.fields {
            let Some(target) = &f.target else { continue };
            let Some(target_id) = resolve_type(index, owner, target) else { continue };
            let shape = f.ty.as_deref().and_then(analyze);
            let many = shape.as_ref().is_some_and(|s| s.many);
            let optional = shape.as_ref().is_some_and(|s| s.optional);
            // A collection navigates to many; a key or a reference points to one (or none).
            let (mult_from, mult_to) = if many { ("1", "0..*") } else { ("0..*", if optional { "0..1" } else { "1" }) };
            let mut r = rel(RelKind::Association, Some(f.name.clone()), format!("{}.{} refers to {target}{}", owner.name, f.name, if f.foreign_key { " (foreign key)" } else { "" }));
            r.mult = (Some(mult_from.into()), Some(mult_to.into()));
            r.site = owner.origins.first().cloned();
            add(&m.symbol, &target_id, r);
        }
    }
    rels
}

/// A field or property referring to a type: composition, aggregation or association.
fn structural(index: &Index, m: &Symbol, owner: &str, target: &str, injected: &BTreeSet<(String, String)>) -> Rel {
    let shape = m.type_text.as_deref().and_then(analyze);
    let target_sym = index.get(target);
    let value_type = target_sym.is_some_and(|t| matches!(t.kind, SymbolKind::Struct | SymbolKind::Enum | SymbolKind::Union | SymbolKind::Record));
    let built_here = m.signature.as_deref().is_some_and(|s| s.contains("= new") || s.contains("=new"));
    let readonly = m.flavors.iter().any(|f| f == "readonly");
    let holding = shape.as_ref().map(|s| s.holding).unwrap_or(Holding::Plain);
    let kind = if injected.contains(&(owner.to_owned(), target.to_owned())) || holding == Holding::Shared {
        RelKind::Aggregation
    } else if m.language == Language::Rust && holding != Holding::Borrowed
        || holding == Holding::Boxed
        || value_type && m.language == Language::Csharp
        || built_here && readonly
    {
        RelKind::Composition
    } else {
        RelKind::Association
    };
    let mult_to = shape.as_ref().map(|s| s.multiplicity().to_owned());
    let label = match shape.as_ref().and_then(|s| s.key.clone()) {
        Some(key) => format!("{} [{key}]", m.name.trim_start_matches('#')),
        None => m.name.trim_start_matches('#').to_owned(),
    };
    let why = match kind {
        RelKind::Composition => "holds it",
        RelKind::Aggregation => "shares it",
        _ => "refers to it",
    };
    Rel {
        kind,
        label: Some(label),
        mult: (None, mult_to.filter(|m| m != "1" || kind != RelKind::Association).or(Some("1".into()))),
        confidence: None,
        evidence: format!("field {}: {} {why}", m.name, m.type_text.as_deref().unwrap_or("?")),
        site: m.origins.first().cloned(),
        count: 1,
    }
}

/// The type a relation endpoint belongs to; external types keep their `ext:` id.
fn lift(index: &Index, id: &str) -> Option<String> {
    if id.starts_with("ext:") {
        return Some(id.to_owned());
    }
    index.owner_type(id).map(|t| t.id.clone())
}

/// A type named in a data model field, preferring the model's namespace.
fn resolve_type(index: &Index, from: &Symbol, name: &str) -> Option<String> {
    let simple = name.rsplit(['.', ':']).next().unwrap_or(name);
    let candidates: Vec<&Symbol> = index.graph.symbols.iter().filter(|s| s.kind.is_type() && !s.external && s.name == simple).collect();
    if candidates.len() == 1 {
        return Some(candidates[0].id.clone());
    }
    let ns = index.namespace_of(&from.id).map(|n| n.id.clone());
    candidates.iter().find(|c| index.namespace_of(&c.id).map(|n| n.id.clone()) == ns).or(candidates.first()).map(|c| c.id.clone())
}
