use std::{path::Path, sync::Arc};

use nodex_core::{Project, borrow_origin::BorrowOriginKind};
use nodex_ir::{
    Access, Concept, FlowEdgeKind, FlowGraph, Language, ValueBasis,
    reading::{ReadingContent, ReadingSite},
    types::TypeShape,
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

fn project() -> Project {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/language-details");
    Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap()
}

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let symbol = project
        .structure()
        .symbols
        .iter()
        .find(|symbol| symbol.language == Language::Rust && symbol.name == name)
        .unwrap()
        .id
        .clone();
    project.flow(&symbol).unwrap()
}

fn sites(flow: &FlowGraph) -> &[ReadingSite] {
    flow.reading.as_deref().unwrap_or_default()
}

fn lookup(flow: &FlowGraph) -> &ReadingSite {
    sites(flow)
        .iter()
        .find(|site| matches!(&site.content, ReadingContent::Detail { borrow: true, .. }))
        .unwrap_or_else(|| panic!("No lookup origin in {:?}", sites(flow)))
}

#[test]
fn return_paths_evaluate_then_clean_up_in_reverse_scope_order() {
    let mut project = project();
    let graph = flow(&mut project, "cleanup_on_return");
    for (code, expected) in [("return inner.read()", &["inner", "outer"][..]), ("outer.read()", &["outer"][..])] {
        let expression = graph.nodes.iter().find(|node| node.code.starts_with(code)
            && node.flavors.iter().any(|flavor| flavor == "return-value")).unwrap();
        let mut next = expression;
        for local in expected {
            let edge = graph.edges.iter().find(|edge| edge.kind == FlowEdgeKind::Exec && edge.from == next.id).unwrap();
            next = graph.node(&edge.to).unwrap();
            assert!(next.label.starts_with(&format!("drop {local} ")), "{}", next.label);
            assert!(!next.label.contains("if still initialized"), "{}", next.label);
        }
        let exit = graph.edges.iter().find(|edge| edge.kind == FlowEdgeKind::Exec && edge.from == next.id).unwrap();
        assert_eq!(graph.node(&exit.to).unwrap().concept, Concept::Return);
    }
    let returned = flow(&mut project, "return_owned_cleanup_value");
    assert!(!returned.nodes.iter().any(|node| node.flavors.iter().any(|flavor| flavor == "implicit-drop")));
}

fn assert_synchronous_spawn(flow: &FlowGraph) {
    let calls: Vec<_> = flow.nodes.iter().filter(|node| node.code.contains("thread::spawn(")).collect();
    assert_eq!(calls.iter().map(|node| node.concept).collect::<Vec<_>>(), [Concept::Call, Concept::Var]);
    assert!(calls.iter().all(|node| !node.label.contains("on another thread")));
    assert!(!sites(flow).iter().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Start a thread" || label == "Schedule a task")));
}

#[test]
fn borrow_cards_show_local_owners_aliases_and_caller_boundaries() {
    let mut project = project();
    let graph = flow(&mut project, "ownership_labels");
    let sources = |code: &str, pin: &str| {
        let node = graph.nodes.iter().find(|node| node.code.starts_with(code)).unwrap();
        &graph.borrows.as_ref().unwrap().iter()
            .find(|site| site.node == node.id && site.pin == pin).unwrap().sources
    };
    let [view] = sources("let view", "var:view").as_slice() else { panic!("expected one source") };
    assert_eq!(view.kind, BorrowOriginKind::Local);
    assert_eq!(view.owner.as_deref(), Some("left"));
    assert_eq!(view.place.as_deref(), Some("left.0"));
    assert_eq!(view.binding.as_ref().unwrap().pin, "var:left");
    assert_eq!(sources("let alias", "var:alias"), sources("let view", "var:view"));
    assert_eq!(sources("let writer", "var:writer")[0].owner.as_deref(), Some("right"));
    let picked = sources("let picked", "use:cursor");
    assert_eq!(picked.len(), 2);
    assert!(picked.iter().any(|source| source.owner.as_deref() == Some("left")));
    assert!(picked.iter().any(|source| source.owner.as_deref() == Some("right")));
    let [caller] = sources("", "var:from_caller").as_slice() else { panic!("expected one source") };
    assert_eq!(caller.kind, BorrowOriginKind::Caller);
    assert_eq!(caller.place.as_deref(), Some("from_caller"));
}

#[test]
fn syntax_reading_explains_patterns_and_the_boundary_of_propagation() {
    let mut project = project();
    assert_synchronous_spawn(&flow(&mut project, "synchronous_spawn"));
    let host = flow(&mut project, "prepare_crew");
    let matched = flow(&mut project, "match_value");
    assert!(sites(&matched).iter().any(|site| matches!(&site.content,
        ReadingContent::Pattern { route_label: Some(label), .. }
        if site.path.is_some() && label == "Case · (LIMIT, value) if value > 0")));
    assert!(
        sites(&host)
            .iter()
            .all(|site| site.basis == ValueBasis::Syntax)
    );
    assert!(
        !sites(&host)
            .iter()
            .any(|site| matches!(site.content, ReadingContent::Paths { .. }))
    );
    let pattern = sites(&host)
        .iter()
        .find_map(|site| match &site.content {
            ReadingContent::Pattern { input, parts, .. } => Some((input, parts)),
            _ => None,
        })
        .unwrap();
    assert!(
        pattern.0.starts_with("Read the referenced tuple"),
        "{pattern:?}"
    );
    assert_eq!(pattern.1[0].binding.as_deref(), Some("host"));
    assert_eq!(pattern.1[0].access, Some(Access::ByValue));
    assert_eq!(pattern.1[1].binding, None);

    let nested = flow(&mut project, "may_exit_callback");
    assert!(sites(&nested).iter().any(|site| matches!(&site.content, ReadingContent::Paths { branches, .. }
        if branches.iter().any(|branch| branch.label == "Propagate" && branch.action == "Return from this callback"))));
    assert!(
        nested
            .nodes
            .iter()
            .all(|node| !node.outputs.iter().any(|pin| pin.id == "error"))
    );

    let old = {
        let mut json = serde_json::to_value(&host).unwrap();
        json.as_object_mut().unwrap().remove("reading");
        serde_json::from_value::<FlowGraph>(json).unwrap()
    };
    assert_eq!(old.nodes, host.nodes);
    assert_eq!(old.edges, host.edges);
    assert!(old.reading.is_none());

    let structure = project.structure();
    for (name, keys) in [
        ("Label", &["trait-contract"][..]),
        (
            "describe",
            &[
                "type-parameters",
                "trait-constraints",
                "where-constraints",
                "lifetime",
            ][..],
        ),
        ("Number", &["derive-traits"][..]),
        (
            "trait_examples",
            &[
                "cfg",
                "generic-parameter-type",
                "trait-object",
                "opaque-return",
            ][..],
        ),
    ] {
        let symbol = structure
            .symbols
            .iter()
            .find(|symbol| symbol.name == name && symbol.language == Language::Rust)
            .unwrap();
        for key in keys {
            assert!(
                symbol
                    .syntax_sites
                    .iter()
                    .flatten()
                    .any(|site| site.key == *key),
                "Missing {key} on {symbol:?}"
            );
        }
    }
}

#[test]
fn range_reading_uses_the_written_bounds_and_links_to_the_operator() {
    let mut project = project();
    let graph = flow(&mut project, "reading_ranges");
    let source = include_str!("../../../fixtures/rosetta/language-details/src/rust_reading.rs");
    let cards: Vec<_> = sites(&graph).iter().filter_map(|site| match &site.content {
        ReadingContent::Detail { label, text, .. } if label.starts_with("Range") => {
            assert_eq!(site.basis, ValueBasis::Syntax);
            assert!(site.path.is_some());
            Some((&source[site.span.start_byte as usize..site.span.end_byte as usize], text.as_str()))
        }
        _ => None,
    }).collect();
    assert_eq!(cards.len(), 6);
    for (operator, phrase) in [
        ("..", "Start at start and stop before end"),
        ("..=", "Start at start and include end"),
        ("..", "Stop before end"),
        ("..=", "Include end as the end bound"),
        ("..", "Start at start with no specified end"),
        ("..", "Neither endpoint is specified"),
    ] {
        assert!(cards.iter().any(|(found, text)| *found == operator && text.starts_with(phrase)), "Missing {operator} / {phrase} in {cards:?}");
    }

    let prefix = flow(&mut project, "reading_inclusive_end");
    let range = prefix.nodes.iter().find(|node| node.flavors.iter().any(|flavor| flavor == "implicit-return")).unwrap();
    assert_eq!(range.concept, Concept::Return);
    let expression = range.expr.as_ref().unwrap();
    assert_eq!(expression.concept, Concept::Compose);
    assert_eq!(expression.label, "range ..=end");
    assert_eq!(expression.children.len(), 1);
    let badges = expression.syntax_sites.as_deref().unwrap();
    assert!(badges.iter().all(|site| site.key != "range-open"));
    let badge = badges.iter().find(|site| site.key == "range-inclusive").unwrap();
    assert_eq!(&source[badge.span.start_byte as usize..badge.span.end_byte as usize], "..=");
    assert!(sites(&prefix).iter().any(|site| site.node == range.id && site.path.is_some()
        && matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Range includes the end")));

    let explicit = flow(&mut project, "reading_explicit_return_range");
    assert!(sites(&explicit).iter().all(|site| !matches!(&site.content,
        ReadingContent::Detail { text, .. } if text.contains("Start at return"))));
}

#[test]
fn boolean_reading_explains_when_checks_run_and_routes_while_let_to_the_body() {
    let mut project = project();
    let checks = flow(&mut project, "reading_boolean_checks");
    let source = include_str!("../../../fixtures/rosetta/language-details/src/rust_reading.rs");
    let paths: Vec<_> = sites(&checks).iter().filter_map(|site| match &site.content {
        ReadingContent::Paths { input, branches, .. } => {
            assert_eq!(site.basis, ValueBasis::Syntax);
            assert!(site.path.is_some());
            assert_eq!(input, "Check left first");
            Some((&source[site.span.start_byte as usize..site.span.end_byte as usize], branches))
        }
        _ => None,
    }).collect();
    assert_eq!(paths.len(), 2);
    for (operator, branches) in paths {
        let skipped_result = if operator == "&&" { "false" } else { assert_eq!(operator, "||"); "true" };
        assert_eq!(branches[0].action, "Skip the right side");
        assert_eq!(branches[0].output, skipped_result);
        assert_eq!(branches[1].action, "Evaluate right()");
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
    }

    let conditions = flow(&mut project, "reading_let_conditions");
    let mut loop_patterns = 0;
    let mut if_patterns = 0;
    for site in sites(&conditions) {
        let node = conditions.node(&site.node).unwrap();
        match &site.content {
            ReadingContent::Pattern { route, .. } if node.concept == Concept::LoopWhile => {
                loop_patterns += 1;
                assert_eq!(route.as_deref(), Some("body"));
                assert!(node.outputs.iter().any(|pin| pin.id == "body"));
            }
            ReadingContent::Pattern { route, .. } if node.concept == Concept::Branch => {
                if_patterns += 1;
                assert_eq!(route.as_deref(), Some("true"));
            }
            ReadingContent::Steps { label, steps } if node.concept == Concept::Branch => {
                assert_eq!(label, "Check each condition in order");
                assert_eq!(steps.len(), 5);
                assert!(steps[0].action.contains("matches Some(value)"));
                assert_eq!(steps[1].action, "Check whether value > minimum is true.");
                assert!(steps[2].action.contains("matches Some(next)"));
                assert_eq!(steps[3].action, "Check whether next > value is true.");
                assert_eq!(steps[4].timing, "If a check fails");
            }
            _ => {}
        }
    }
    assert_eq!(loop_patterns, 2);
    assert_eq!(if_patterns, 2);
    assert_eq!(sites(&conditions).iter().filter(|site| matches!(site.content, ReadingContent::Steps { .. })).count(), 2);
}

#[test]
fn let_chain_reads_the_outer_value_only_before_a_shadowing_binding() {
    let mut project = project();
    let graph = flow(&mut project, "reading_shadowed_conditions");
    let condition = |snippet: &str| graph.nodes.iter().find(|node| node.code.starts_with(snippet)).unwrap();
    let incoming = |node: &nodex_ir::FlowNode, pin: &str| {
        graph.edges.iter().find(|edge| edge.kind == FlowEdgeKind::Data && edge.to == node.id && edge.to_pin == pin)
    };
    assert!(incoming(condition("if let Some(x) = input"), "use:x").is_none());
    assert!(incoming(condition("while let Some(y)"), "use:y").is_none());
    let scrutinee = incoming(condition("if let Some(x) = Some(x)"), "use:x").unwrap();
    assert_eq!(scrutinee.from_pin, "var:x");
    assert_eq!(graph.node(&scrutinee.from).unwrap().concept, Concept::Entry);
}

#[test]
fn let_else_reading_follows_pattern_match_and_mismatch_routes() {
    let mut project = project();
    let graph = flow(&mut project, "reading_let_else");
    let mut patterns = Vec::new();
    for site in sites(&graph) {
        let ReadingContent::Paths { input, branches, .. } = &site.content else { continue };
        assert_eq!(site.basis, ValueBasis::Syntax);
        let node = graph.node(&site.node).unwrap();
        assert_eq!(node.concept, Concept::Destructure);
        assert_eq!(branches[0].label, "Pattern matches");
        assert!(branches[0].action.contains("Any names bound by the pattern"));
        assert_eq!(branches[1].label, "Pattern mismatch");
        for (branch, pin) in branches.iter().zip(["out", "else"]) {
            assert_eq!(branch.pin.as_deref(), Some(pin));
            assert!(node.outputs.iter().any(|output| output.id == pin));
        }
        patterns.push(input.as_str());
    }
    assert_eq!(patterns, ["Check whether input matches Some(value)", "Check whether candidate matches Some(_)"]);

    let parent = flow(&mut project, "reading_let_else_closure");
    assert!(sites(&parent).iter().all(|site| !matches!(site.content, ReadingContent::Paths { .. })));
    let closure = flow(&mut project, "convert_with_default closure");
    let branch_site = sites(&closure).iter().find(|site| matches!(&site.content,
        ReadingContent::Paths { deferred: false, branches, .. }
        if branches.iter().any(|branch| branch.pin.as_deref() == Some("else")))).unwrap();
    assert!(branch_site.path.is_none());
    let statement = closure.node(&branch_site.node).unwrap();
    assert_eq!(statement.concept, Concept::Destructure);
    for pin in ["out", "else"] {
        assert!(closure.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Exec
            && edge.from == statement.id && edge.from_pin == pin));
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_reading_connects_lookup_paths_patterns_and_origins() {
    let mut project = project();
    let before = flow(&mut project, "protected");
    let thread = before.nodes.iter().find(|node| node.code.starts_with("let thread =")).unwrap();
    assert_eq!(thread.concept, Concept::Var);
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    let wrapped = flow(&mut project, "split_through_wrapper");
    for pin in ["var:first", "var:second"] {
        let sources = &wrapped.borrows.iter().flatten().find(|site| site.pin == pin).unwrap().sources;
        assert!(sources.iter().all(|source| source.kind == BorrowOriginKind::Unknown), "{sources:?}");
    }
    let capture = flow(&mut project, "capture_only_field");
    assert!(sites(&capture).iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("Captures pair.1 by copying the value"))), "{:?}", sites(&capture));
    assert!(capture.edges.iter().any(|edge| edge.to_pin == "use:pair" && edge.access.is_none()
        && capture.node(&edge.to).unwrap().code.starts_with("let callback")));
    let effects = flow(&mut project, "effects_and_slices");
    assert!(sites(&effects).iter().any(|site| matches!(&site.content, ReadingContent::Steps { steps, .. }
        if steps.iter().any(|step| step.action.contains("leave it empty (None)")))));
    assert!(sites(&effects).iter().any(|site| matches!(&site.content, ReadingContent::Paths { branches, .. }
        if branches.iter().any(|branch| branch.label == "Key missing"))));
    assert!(effects.borrows.iter().flatten().any(|site| site.pin == "var:first" && site.sources.iter()
        .any(|origin| origin.place.as_deref() == Some("values[..1][0]"))));
    let ordinary = flow(&mut project, "ordinary_effect_names");
    let ordinary_calls: Vec<_> = ordinary.nodes.iter().filter(|node|
        ["drop(3)", "exit()", "sleep()", "Counter.send(4)"].iter().any(|call| node.code.starts_with(call))
    ).collect();
    assert_eq!(ordinary_calls.len(), 4);
    assert!(ordinary_calls.iter().all(|node| node.concept == Concept::Call));
    assert!(sites(&ordinary).is_empty());
    let after = flow(&mut project, "protected");
    assert_eq!(after.nodes.iter().find(|node| node.id == thread.id).unwrap().concept, Concept::Spawn);
    assert_synchronous_spawn(&flow(&mut project, "synchronous_spawn"));
    let host = flow(&mut project, "prepare_crew");
    let statement = host
        .nodes
        .iter()
        .find(|node| node.code.starts_with("let host"))
        .unwrap();
    let paths = sites(&host)
        .iter()
        .find(|site| {
            site.node == statement.id && matches!(site.content, ReadingContent::Paths { .. })
        })
        .unwrap();
    let ReadingContent::Paths {
        input,
        deferred,
        branches,
        result: Some(result),
        ..
    } = &paths.content
    else {
        panic!("{paths:?}")
    };
    assert!(*deferred);
    assert_eq!(input, "Look up crew in encounter.crew_index");
    assert_eq!(branches[0].label, "Found");
    assert_eq!(branches[0].action, "Copy the first tuple item as host");
    assert_eq!(branches[0].output, "Some(host)");
    assert_eq!(
        (&*branches[1].label, &*branches[1].output),
        ("Missing", "None")
    );
    assert_eq!(
        result.shape,
        TypeShape::Named {
            name: "Option".into(),
            arguments: vec![TypeShape::scalar("UIntSize")]
        }
    );

    let pattern = sites(&host)
        .iter()
        .find(|site| matches!(&site.content, ReadingContent::Pattern { .. }))
        .unwrap();
    let ReadingContent::Pattern {
        source: Some(source),
        parts,
        input_type: Some(input_type),
        ..
    } = &pattern.content
    else {
        panic!("{pattern:?}")
    };
    assert_eq!(source, &lookup(&host).span);
    assert_eq!(
        input_type.shape,
        TypeShape::Indirect {
            pointer: false,
            target: Box::new(TypeShape::Tuple {
                items: vec![TypeShape::scalar("UIntSize"), TypeShape::scalar("UIntSize")],
            })
        }
    );
    assert_eq!(
        parts[0].type_info.as_ref().unwrap().native.as_deref(),
        Some("usize")
    );
    assert_eq!(parts[0].access, Some(Access::Copy));
    assert!(
        parts[0].span.start_byte
            > statement
                .outputs
                .iter()
                .find(|pin| pin.label == "host")
                .unwrap()
                .declaration
                .as_ref()
                .unwrap()
                .start_byte
    );
    assert_eq!(parts[1].projection, ".1");
    assert_eq!(parts[1].binding, None);

    for (name, kind, borrowed) in [
        (
            "prepare_crew",
            BorrowOriginKind::Local,
            "encounter.crew_index[crew]",
        ),
        (
            "from_caller_only",
            BorrowOriginKind::Caller,
            "encounter.crew_index[crew]",
        ),
        ("find_local", BorrowOriginKind::Local, "owner[crew]"),
        ("keep_borrow", BorrowOriginKind::Caller, "map[key]"),
    ] {
        let flow = flow(&mut project, name);
        let get = lookup(&flow);
        let origin = project
            .borrow_origin(&flow.symbol, &get.node, None, get.path.as_deref(), 0)
            .unwrap();
        assert!(
            origin
                .origins
                .iter()
                .any(|entry| entry.kind == kind && entry.borrowed.as_deref() == Some(borrowed)),
            "{name}: {:?}",
            origin.origins
        );
        if name == "keep_borrow" {
            let map = sites(&flow)
                .iter()
                .find(|site| matches!(site.content, ReadingContent::Paths { .. }))
                .unwrap();
            let origins = project
                .borrow_origin(&flow.symbol, &map.node, None, map.path.as_deref(), 0)
                .unwrap();
            assert_eq!(
                origins.origins[0].borrowed.as_deref(),
                Some(borrowed),
                "{:?}",
                origins.origins
            );
        }
    }

    let custom = flow(&mut project, "not_a_lookup");
    for (name, kind, expected) in [
        ("changed_capture", BorrowOriginKind::Unknown, None),
        ("unpack_entry", BorrowOriginKind::Caller, Some("map[key].0")),
    ] {
        let flow = flow(&mut project, name);
        let output = sites(&flow)
            .iter()
            .find(|site| {
                matches!(&site.content,
            ReadingContent::Detail { label, .. } if label == "Callback returns a reference")
            })
            .unwrap();
        let origins = project
            .borrow_origin(&flow.symbol, &output.node, None, output.path.as_deref(), 0)
            .unwrap();
        assert!(
            origins
                .origins
                .iter()
                .all(|origin| origin.kind == kind && origin.borrowed.as_deref() == expected),
            "{name}: {:?}",
            origins.origins
        );
    }
    let split = flow(&mut project, "borrow_split");
    assert!(sites(&split).iter().any(|site| matches!(&site.content, ReadingContent::Pattern { parts, .. }
        if parts.iter().any(|part| part.projection == "[..1]" && part.access == Some(Access::Borrow)))));
    let matched = flow(&mut project, "match_value");
    assert!(sites(&matched).iter().any(|site| matches!(&site.content, ReadingContent::Pattern { parts, .. }
        if parts.iter().any(|part| part.condition.as_deref() == Some("LIMIT") && part.binding.is_none())
        && parts.iter().any(|part| part.binding.as_deref() == Some("value")))));
    assert!(
        !sites(&custom)
            .iter()
            .any(|site| matches!(site.content, ReadingContent::Paths { .. }))
    );
    assert!(
        custom
            .nodes
            .iter()
            .all(|node| !node.flavors.iter().any(|flavor| flavor == "may-panic"))
    );
    let shadowed = flow(&mut project, "shadowed_callback");
    assert!(sites(&shadowed).iter().any(|site| matches!(&site.content, ReadingContent::Paths { branches, .. }
        if branches[0].action == "Run the callback with the value" && branches[0].output == "Some(host)")));
    let view = flow(&mut project, "opaque_receiver");
    let get = lookup(&view);
    let origins = project
        .borrow_origin(&view.symbol, &get.node, None, get.path.as_deref(), 0)
        .unwrap();
    assert_eq!(origins.origins[0].kind, BorrowOriginKind::Unknown);

    let reborrow = flow(&mut project, "reborrow_entry");
    let callback = sites(&reborrow)
        .iter()
        .find(|site| {
            matches!(&site.content, ReadingContent::Detail { label, .. }
        if label == "Callback returns a reference")
        })
        .unwrap();
    let origins = project
        .borrow_origin(
            &reborrow.symbol,
            &callback.node,
            None,
            callback.path.as_deref(),
            0,
        )
        .unwrap();
    assert_eq!(
        origins.origins[0].borrowed.as_deref(),
        Some("map[key].0"),
        "{:?}",
        origins.origins
    );
    let nested = flow(&mut project, "may_exit_callback");
    assert!(sites(&nested).iter().any(|site| site.basis == ValueBasis::TypeAnalysis
        && matches!(&site.content, ReadingContent::Paths { branches, .. } if branches.iter().any(|branch| {
            branch.label == "Missing" && branch.action == "Return from this callback"
        }))));
    assert!(sites(&nested).iter().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Prepare an iterator step")));

    for (name, labels) in [
        (
            "protected",
            &[
                "Share ownership",
                "Start a thread",
                "Wait for a thread",
                "Acquire protected access",
            ][..],
        ),
        (
            "deferred",
            &["Async function", "Create a future", "Await a result"][..],
        ),
        (
            "macros",
            &["Build a list", "Check a condition", "Format text"][..],
        ),
    ] {
        let flow = flow(&mut project, name);
        for label in labels {
            assert!(sites(&flow).iter().any(|site| matches!(&site.content, ReadingContent::Detail { label: actual, .. } if actual == label)),
                "{name} missing {label}: {:?}", sites(&flow));
        }
    }
    let macros = flow(&mut project, "macros");
    assert!(sites(&macros).iter().any(|site| matches!(
        &site.content,
        ReadingContent::Detail {
            definition: Some(_),
            ..
        }
    )));
    let split = flow(&mut project, "split");
    assert!(sites(&split).iter().any(|site| matches!(&site.content, ReadingContent::Pattern { parts, .. }
        if parts.iter().any(|part| part.projection == "[..mid]" && part.access == Some(Access::BorrowMut)))));
    let fallible = flow(&mut project, "fallible");
    assert!(sites(&fallible).iter().any(|site| matches!(&site.content, ReadingContent::Paths { branches, .. } if branches.len() == 3)));
    assert!(sites(&fallible).iter().any(|site| matches!(&site.content, ReadingContent::Pattern { route, .. } if route.as_deref() == Some("out"))));
    let defaults = flow(&mut project, "defaults");
    assert!(sites(&defaults).iter().any(
        |site| matches!(&site.content, ReadingContent::Paths { branches, .. }
        if branches[0].action.starts_with("Evaluate the default"))
    ));
    assert!(sites(&defaults).iter().any(
        |site| matches!(&site.content, ReadingContent::Paths { branches, .. }
        if branches[0].action.contains("skip the fallback"))
    ));

    project.set_semantic_status(
        Language::Rust,
        nodex_ir::semantic::SemanticState::Loading,
        "refreshing",
    );
    let get = lookup(&host);
    let origins = project
        .borrow_origin(&host.symbol, &get.node, None, get.path.as_deref(), 0)
        .unwrap();
    assert_eq!(origins.origins[0].kind, BorrowOriginKind::Unknown);
}
