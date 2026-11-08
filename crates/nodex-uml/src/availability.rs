//! Which diagrams have something to show for a scope, and which one to suggest.

use crate::index::Index;
use crate::scope::Scoped;
use crate::{UmlAvailability, UmlError, UmlInput, UmlKind, UmlKindAvailability, UmlOptions, UmlScope, build};

pub fn availability(input: &UmlInput, scope: &UmlScope) -> Result<UmlAvailability, UmlError> {
    let index = Index::new(input.graph, input.knowledge);
    let scoped = Scoped::resolve(&index, scope)?;
    let opts = UmlOptions::default();
    let types = scoped.types.iter().filter(|t| !index.is_test(t) && index.get(t).is_some_and(|s| s.kind.is_type())).count();
    let mut kinds = Vec::new();
    let mut suggested = None;

    kinds.push(UmlKindAvailability {
        kind: UmlKind::Class,
        available: types > 0,
        reason: if types > 0 { format!("{types} types") } else { "No types in this scope".into() },
    });
    let packages = build(input, UmlKind::Package, scope, &opts).map(|d| d.elements.iter().filter(|e| e.kind == "package").count()).unwrap_or(0);
    kinds.push(UmlKindAvailability {
        kind: UmlKind::Package,
        available: packages > 0,
        reason: match packages {
            0 => "No code in this scope".into(),
            1 => "1 package".into(),
            n => format!("{n} packages"),
        },
    });
    let components = build(input, UmlKind::Component, scope, &opts).map(|d| d.elements.iter().filter(|e| e.kind == "component").count()).unwrap_or(0);
    kinds.push(UmlKindAvailability {
        kind: UmlKind::Component,
        available: components > 0,
        reason: match components {
            0 => "No code in this scope".into(),
            1 => "1 component".into(),
            n => format!("{n} components"),
        },
    });
    let (functions, root) = crate::roots::roots(&index, &scoped, scope, UmlKind::Activity, &opts);
    let root = root.and_then(|r| index.get(&r));
    let about = |what: &str| match (functions.len(), root) {
        (0, _) => "No function with a body in this scope".to_owned(),
        (1, Some(r)) => format!("{what} of {}", r.name),
        (n, Some(r)) => format!("{what} of {} (or {} more functions)", r.name, n - 1),
        (n, None) => format!("{n} functions"),
    };
    kinds.push(UmlKindAvailability { kind: UmlKind::Activity, available: !functions.is_empty(), reason: about("How it runs: logic") });
    kinds.push(UmlKindAvailability { kind: UmlKind::Sequence, available: !functions.is_empty(), reason: about("Who it calls") });
    let entries = crate::usecase::count(&index, &scoped, input);
    kinds.push(UmlKindAvailability {
        kind: UmlKind::UseCase,
        available: entries > 0,
        reason: match entries {
            0 => "No entry points here (routes, UI actions, commands, jobs, handlers)".into(),
            1 => "1 use case".into(),
            n => format!("{n} use cases"),
        },
    });
    let machines = crate::state::offered(&index, &scoped, input);
    kinds.push(UmlKindAvailability {
        kind: UmlKind::State,
        available: !machines.is_empty(),
        reason: match machines.as_slice() {
            [] => "No state variable or declared state machine here".into(),
            [(one, _)] => format!("States of {one}"),
            [(first, _), rest @ ..] => format!("States of {first} (or {} more)", rest.len()),
        },
    });
    // A function (selected, or the one around the selection) is best read as its logic, or as
    // its conversation with other parts when it is an entry point that calls out.
    let function_scope = match scope {
        UmlScope::Range { .. } => root.is_some() && scoped.types.len() <= 1,
        UmlScope::Symbol { id } => index.get(id).is_some_and(|s| s.has_flow),
        _ => false,
    };

    // The most useful view: how the parts fit for a project, how code is organized for a
    // folder with many packages, the classes otherwise.
    if function_scope && let Some(r) = root {
        let info = index.info.get(r.id.as_str());
        let entry = info.and_then(|i| i.entry.as_ref()).is_some_and(|e| !matches!(e.kind, nodex_ir::knowledge::EntryKind::Test));
        let talks = info.is_some_and(|i| i.capabilities.iter().any(|c| matches!(c.as_str(), "DB_READ" | "DB_WRITE" | "NETWORK_IO")));
        suggested = Some(if entry || talks { UmlKind::Sequence } else { UmlKind::Activity });
    } else if let (UmlScope::Symbol { id }, Some((_, score))) = (scope, machines.first())
        && index.get(id).is_some_and(|s| s.kind.is_type())
        && *score >= 75.0
    {
        // A type whose behavior is a state machine.
        suggested = Some(UmlKind::State);
    } else if scoped.broad && components > 1 && matches!(scope, UmlScope::Project) {
        suggested = Some(UmlKind::Component);
    } else if scoped.broad && !matches!(scope, UmlScope::Project) && entries >= 3 {
        // A folder of controllers, handlers or pages: what it lets people do.
        suggested = Some(UmlKind::UseCase);
    } else if scoped.broad && packages >= 3 {
        suggested = Some(UmlKind::Package);
    } else if types > 0 {
        suggested = Some(UmlKind::Class);
    } else if packages > 0 {
        suggested = Some(UmlKind::Package);
    }
    kinds.sort_by_key(|k| UmlKind::ALL.iter().position(|x| *x == k.kind));
    Ok(UmlAvailability { title: scoped.title, kinds, suggested })
}
