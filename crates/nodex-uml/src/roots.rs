//! Which function an activity or sequence diagram starts from.
//!
//! A selection inside a function, or a function symbol, names it. A wider scope offers the
//! functions it contains as candidates, best first: entry points (routes, UI actions, message
//! handlers, commands, jobs, `main`), then the functions with the most logic.

use nodex_ir::knowledge::EntryKind;
use nodex_ir::{Symbol, SymbolKind};

use crate::index::Index;
use crate::scope::Scoped;
use crate::{UmlCandidate, UmlKind, UmlOptions, UmlScope};

const MAX_CANDIDATES: usize = 60;

fn entry_score(kind: EntryKind) -> f32 {
    match kind {
        EntryKind::Route => 100.0,
        EntryKind::Ui => 95.0,
        EntryKind::Message | EntryKind::Command => 90.0,
        EntryKind::Job => 85.0,
        EntryKind::EventHandler => 80.0,
        EntryKind::Main => 75.0,
        EntryKind::ServerStart => 60.0,
        EntryKind::Export => 50.0,
        EntryKind::Framework => 40.0,
        EntryKind::Test => 10.0,
    }
}

/// Display name of a function: `OrderService.CancelOrderAsync`, `orders.cancelOrder`.
pub fn function_label(index: &Index, s: &Symbol) -> String {
    let owner = index.get(s.parent.as_deref().unwrap_or("")).filter(|p| p.kind.is_type() || matches!(p.kind, SymbolKind::Function | SymbolKind::Method));
    let name = s.name.trim_end_matches(".get").trim_end_matches(".set");
    match owner {
        Some(o) if s.kind == SymbolKind::Constructor => format!("new {}", o.name),
        Some(o) => format!("{}.{name}", o.name),
        None => match index.namespace_of(&s.id) {
            Some(m) if m.kind == SymbolKind::Module => format!("{}.{name}", module_name(m)),
            _ => name.to_owned(),
        },
    }
}

/// A module's short name: its file name without extension, its last path segment.
pub fn module_name(m: &Symbol) -> String {
    let q = m.qualified_name.as_str();
    let last = q.rsplit(['/', ':', '.']).find(|p| !p.is_empty()).unwrap_or(q);
    if last == "crate" || last == "(module)" {
        // The crate root and file modules are named after their file (`main`, `lib`).
        let file = m.origins.first().map(|o| o.file.as_str()).unwrap_or("");
        let stem = file.rsplit('/').next().unwrap_or(file).split('.').next().unwrap_or("");
        return if stem.is_empty() { m.name.clone() } else { stem.to_owned() };
    }
    last.to_owned()
}

/// The candidates of a scope, best first, and the one the diagram uses.
pub fn roots(index: &Index, scoped: &Scoped, scope: &UmlScope, kind: UmlKind, opts: &UmlOptions) -> (Vec<UmlCandidate>, Option<String>) {
    let range = match scope {
        UmlScope::Range { file, start_byte, end_byte } => Some((file.as_str(), *start_byte, (*end_byte).max(*start_byte + 1))),
        _ => None,
    };
    let target_fn = match scope {
        UmlScope::Symbol { id } => index.get(id).filter(|s| s.has_flow).map(|s| s.id.clone()),
        _ => None,
    };
    let mut list: Vec<UmlCandidate> = Vec::new();
    for id in &scoped.symbols {
        let Some(s) = index.get(id).filter(|s| s.has_flow && !s.external) else { continue };
        if opts.hide_tests && scoped.broad && index.is_test(id) {
            continue;
        }
        let info = index.info.get(id.as_str());
        let complexity = info.map(|i| i.complexity).unwrap_or(1);
        let entry = info.and_then(|i| i.entry.as_ref());
        let mut score = complexity.min(30) as f32 * if kind == UmlKind::Activity { 3.0 } else { 1.5 };
        score += entry.map(|e| entry_score(e.kind)).unwrap_or(0.0);
        score += info.map(|i| i.capabilities.len() as f32 * if kind == UmlKind::Sequence { 4.0 } else { 1.0 }).unwrap_or(0.0);
        if s.flavors.iter().any(|f| f == "public" || f == "export" || f == "pub") {
            score += 5.0;
        }
        // Nested callbacks and lambdas are rarely what a reader looks for.
        if index.get(s.parent.as_deref().unwrap_or("")).is_some_and(|p| matches!(p.kind, SymbolKind::Function | SymbolKind::Method)) {
            score -= 10.0;
        }
        if matches!(s.kind, SymbolKind::Constructor) || s.name.ends_with(".get") || s.name.ends_with(".set") {
            score -= 15.0;
        }
        // The innermost function around the selection wins.
        if let Some((file, start, end)) = range
            && let Some(o) = s.origins.iter().find(|o| o.file == file && o.start_byte <= start && end <= o.end_byte)
        {
            score += 1000.0 + 1000.0 / (1.0 + (o.end_byte - o.start_byte) as f32 / 100.0);
        }
        if target_fn.as_deref() == Some(id.as_str()) {
            score += 5000.0;
        }
        let mut detail = Vec::new();
        if let Some(e) = entry {
            detail.push(e.label.clone());
        }
        if complexity > 1 {
            detail.push(format!("complexity {complexity}"));
        }
        list.push(UmlCandidate { id: id.clone(), label: function_label(index, s), detail: (!detail.is_empty()).then(|| detail.join(" · ")), score });
    }
    list.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.label.cmp(&b.label)));
    list.truncate(MAX_CANDIDATES);
    let chosen = match &opts.focus {
        Some(f) if index.get(f).is_some_and(|s| s.has_flow) => Some(f.clone()),
        _ => list.first().map(|c| c.id.clone()),
    };
    (list, chosen)
}
