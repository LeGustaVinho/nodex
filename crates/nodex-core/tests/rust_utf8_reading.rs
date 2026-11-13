use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    FlowEdgeKind, FlowGraph, Language, Span, ValueBasis,
    reading::{ReadingBranch, ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/utf8-reading/src/lib.rs");

fn project() -> Project {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/utf8-reading");
    Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap()
}

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project
        .structure()
        .symbols
        .iter()
        .find(|symbol| symbol.name == name)
        .unwrap()
        .id
        .clone();
    project.flow(&id).unwrap()
}

fn source(span: &Span) -> &str {
    assert_eq!(span.file, "src/lib.rs");
    &SOURCE[span.start_byte as usize..span.end_byte as usize]
}

fn conversion(graph: &FlowGraph) -> Option<(&ReadingSite, &[ReadingBranch])> {
    graph.reading.iter().flatten().find_map(|site| {
        let ReadingContent::Paths { branches, .. } = &site.content else {
            return None;
        };
        (branches.first()?.label == "Valid UTF-8").then_some((site, branches.as_slice()))
    })
}

#[test]
fn utf8_spelling_alone_does_not_establish_the_conversion() {
    let mut project = project();
    for name in ["borrowed_match", "owned", "lossy", "local_decoder"] {
        assert!(conversion(&flow(&mut project, name)).is_none());
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_distinguishes_owned_borrowed_and_lossy_utf8_conversions() {
    let mut project = project();
    let before = flow(&mut project, "snapshot");
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());

    for (function, token, valid, invalid, valid_action, invalid_action) in [
        (
            "borrowed",
            "decode_borrowed",
            "Ok(&str)",
            "Err(Utf8Error)",
            "same bytes",
            "original bytes remain unchanged",
        ),
        (
            "borrowed_primitive",
            "from_utf8",
            "Ok(&str)",
            "Err(Utf8Error)",
            "same bytes",
            "original bytes remain unchanged",
        ),
        (
            "borrowed_match",
            "from_utf8",
            "Ok(&str)",
            "Err(Utf8Error)",
            "shared reference",
            "validation stopped",
        ),
        (
            "owned",
            "from_utf8",
            "Ok(String)",
            "Err(FromUtf8Error)",
            "without copying",
            "Recover it with into_bytes()",
        ),
        (
            "snapshot",
            "from_utf8",
            "Ok(String)",
            "Err(FromUtf8Error)",
            "vector's buffer",
            "original vector",
        ),
        (
            "lossy",
            "from_utf8_lossy",
            "Cow::Borrowed(&str)",
            "Cow::Owned(String)",
            "without allocating",
            "U+FFFD",
        ),
        (
            "decode_number",
            "from_utf8_lossy",
            "Cow::Borrowed(&str)",
            "Cow::Owned(String)",
            "without allocating",
            "U+FFFD",
        ),
    ] {
        let graph = flow(&mut project, function);
        let (site, branches) =
            conversion(&graph).unwrap_or_else(|| panic!("{function}: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(source(&site.span), token);
        assert!(branches.iter().all(|branch| branch.pin.is_none()));
        assert_eq!(branches[0].output, valid);
        assert_eq!(branches[1].output, invalid);
        assert!(branches[0].action.contains(valid_action));
        assert!(branches[1].action.contains(invalid_action));
    }
    assert!(conversion(&flow(&mut project, "local_decoder")).is_none());

    let after = flow(&mut project, "snapshot");
    let execution = |graph: &FlowGraph| {
        graph
            .edges
            .iter()
            .filter(|edge| edge.kind == FlowEdgeKind::Exec)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(execution(&before), execution(&after));
    let parse = flow(&mut project, "decode_number");
    assert!(
        parse
            .reading
            .iter()
            .flatten()
            .any(|site| source(&site.span) == "?"
                && matches!(&site.content, ReadingContent::Paths { branches, .. }
            if branches.iter().any(|branch| branch.pin.as_deref() == Some("error"))))
    );
}
