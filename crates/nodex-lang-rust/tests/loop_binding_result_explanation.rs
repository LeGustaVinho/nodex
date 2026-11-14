use nodex_ir::{Concept, reading::ReadingContent};
use nodex_lang_rust::lower_source;

#[test]
fn loop_initializer_explains_value_and_preserves_general_loop_detail() {
    let source = "fn f() { let x = loop { break 42; }; let y = (loop { break 7; }); loop { break; } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Bind the result of a loop")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for site in facts {
        let node = flow.nodes.iter().find(|node| node.id == site.node).unwrap();
        assert_eq!(node.concept, Concept::LoopInfinite);
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "let");
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        assert!(text.contains("break 42"));
        assert!(text.contains("targeting this loop"));
        assert!(text.contains("never completes"));
        assert!(text.contains("different labeled loop or block"));
    }
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Repeat the loop body")));
}
