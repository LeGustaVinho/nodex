use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{FlowGraph, Language, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

const SOURCE: &str = include_str!("../../../fixtures/rosetta/vec-repeat/src/lib.rs");

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project.structure().symbols.iter().find(|symbol| symbol.name == name).unwrap().id.clone();
    project.flow(&id).unwrap()
}

fn repeat(graph: &FlowGraph) -> Option<&ReadingSite> {
    graph.reading.iter().flatten().find(|site| matches!(&site.content,
        ReadingContent::Steps { label, .. } if label == "Build a vector by repetition"))
}

#[test]
#[ignore = "needs rust-analyzer"]
fn scip_vec_repeat_explains_standard_repetition_and_preserves_macro_details() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/vec-repeat");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    assert!(repeat(&flow(&mut project, "integrate_heat")).is_none());

    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    let cases = [
        ("integrate_heat", "Vec::new()", "count", "vec"),
        ("advance_pulses", "false", "ships.len()", "vec"),
        ("with_generic_commas", "value::<u32, u64>()", "count", "repeat_vec"),
        ("zero_count", "{ *evaluations += 1; 5 }", "0", "std::vec"),
    ];
    let source = |span: &nodex_ir::Span| &SOURCE[span.start_byte as usize..span.end_byte as usize];
    for (function, value, length, macro_name) in cases {
        let graph = flow(&mut project, function);
        let site = repeat(&graph).unwrap_or_else(|| panic!("No repeat card for {function}: {:?}", graph.reading));
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
        assert_eq!(source(&site.span), ";");
        let ReadingContent::Steps { steps, .. } = &site.content else { unreachable!() };
        assert_eq!(steps.iter().map(|step| source(&step.span)).collect::<Vec<_>>(), [value, length, ";"]);
        assert!(steps[0].action.contains("once, even when the length is zero"));
        assert!(steps[1].action.contains("at runtime"));
        assert!(steps[2].action.contains("Clone implementation"));
        assert!(steps[2].action.contains("can share data"));
        assert!(steps[2].action.contains("leaves the vector empty"));
        assert!(graph.reading.iter().flatten().any(|detail| detail.node == site.node && source(&detail.span) == macro_name
            && matches!(&detail.content, ReadingContent::Detail { label, .. } if label == "Build a list")));
    }

    let list = flow(&mut project, "list_form");
    assert!(repeat(&list).is_none());
    assert!(list.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { text, .. } if text == "This macro creates a Vec with the specified items.")));
    let local = flow(&mut project, "local_vec");
    assert!(repeat(&local).is_none());
    assert!(local.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Macro invocation")));
}
