use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/numeric-conversions/src/lib.rs");

fn project() -> Project {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/numeric-conversions");
    Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap()
}

fn flow(project: &mut Project, function: &str) -> FlowGraph {
    let symbol = project
        .structure()
        .symbols
        .iter()
        .find(|symbol| symbol.name == function)
        .unwrap()
        .id
        .clone();
    project.flow(&symbol).unwrap()
}

fn is_conversion(site: &ReadingSite) -> bool {
    match &site.content {
        ReadingContent::Detail { label, .. } => {
            label.starts_with("Convert ") || label.starts_with("Try converting ")
        }
        ReadingContent::Paths { input, .. } => input.starts_with("Try converting "),
        _ => false,
    }
}

fn conversion_at<'a>(flow: &'a FlowGraph, call: &str) -> &'a ReadingSite {
    let call_start = SOURCE.find(call).unwrap();
    let callee = call.split_once('(').unwrap().0;
    let method = callee.rsplit("::").next().unwrap();
    let start = call_start + callee.len() - method.len();
    let end = start + method.len();
    let site = flow
        .reading
        .iter()
        .flatten()
        .find(|site| {
            is_conversion(site)
                && site.span.file == "src/lib.rs"
                && site.span.start_byte as usize == start
                && site.span.end_byte as usize == end
        })
        .unwrap_or_else(|| panic!("missing reading for {call} in {:?}", flow.reading));
    assert_eq!(site.basis, ValueBasis::TypeAnalysis);
    site
}

#[test]
fn numeric_conversion_spelling_alone_does_not_establish_conversion_behavior() {
    let mut project = project();
    for function in [
        "in_bucket",
        "checked_count",
        "narrow",
        "shadowed_conversion",
    ] {
        let graph = flow(&mut project, function);
        assert!(!graph.reading.iter().flatten().any(is_conversion));
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_distinguishes_primitive_from_checked_and_custom_conversions() {
    let mut project = project();
    let before = flow(&mut project, "checked_count");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (function, call, expected_label, destination) in [
        (
            "in_bucket",
            "u64::from(bucket)",
            "Convert u16 to u64",
            "u64",
        ),
        ("to_float", "f64::from(value)", "Convert i16 to f64", "f64"),
    ] {
        let graph = flow(&mut project, function);
        let ReadingContent::Detail { label, text, .. } = &conversion_at(&graph, call).content
        else {
            panic!("expected direct conversion for {call}");
        };
        assert_eq!(label, expected_label);
        assert!(text.contains(&format!("return it directly as {destination}")));
    }

    let graph = flow(&mut project, "narrow");
    let call = "<u8 as std::convert::TryFrom<i16>>::try_from(value)";
    let ReadingContent::Paths {
        input, branches, ..
    } = &conversion_at(&graph, call).content
    else {
        panic!("expected checked conversion for {call}");
    };
    assert_eq!(input, "Try converting i16 to u8");
    assert_eq!(branches.len(), 2);
    assert!(branches[0].action.contains("0 through 255"));
    assert_eq!(branches[0].output, "Ok(u8 value)");
    assert_eq!(branches[1].output, "Err(TryFromIntError)");
    assert!(branches.iter().all(|branch| branch.pin.is_none()));

    for (function, call, expected_label, range, output) in [
        (
            "checked_count",
            "u16::try_from(records.len())",
            "Try converting usize to u16",
            "0 through 65535",
            "Ok(u16 value)",
        ),
        (
            "to_platform_size",
            "usize::try_from(value)",
            "Try converting u64 to usize",
            "the range of usize on the target platform",
            "Ok(usize value)",
        ),
    ] {
        let graph = flow(&mut project, function);
        let ReadingContent::Detail { label, text, .. } = &conversion_at(&graph, call).content
        else {
            panic!("expected platform-dependent conversion for {call}");
        };
        assert_eq!(label, expected_label);
        for term in [range, "target platform", output, "Err(TryFromIntError)"] {
            assert!(text.contains(term), "{text}");
        }
    }

    let after = flow(&mut project, "checked_count");
    let try_from = conversion_at(&after, "u16::try_from(records.len())");
    let expect = after
        .reading
        .iter()
        .flatten()
        .find(|site| {
            let spelling = &SOURCE[site.span.start_byte as usize..site.span.end_byte as usize];
            spelling == r#"u16::try_from(records.len()).expect("too many records")"#
                && matches!(&site.content, ReadingContent::Paths { branches, .. }
                if branches.iter().any(|branch| branch.action == "Panic"))
        })
        .expect("expect retains its own panic explanation");
    assert_eq!(try_from.node, expect.node);
    assert_ne!(try_from.path, expect.path);
    let execution = |flow: &FlowGraph| {
        flow.edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after));

    for function in [
        "other_conversions",
        "local_conversions",
        "shadowed_conversion",
    ] {
        let graph = flow(&mut project, function);
        assert!(
            !graph.reading.iter().flatten().any(is_conversion),
            "unexpected numeric conversion in {function}: {:?}",
            graph.reading,
        );
    }
}
