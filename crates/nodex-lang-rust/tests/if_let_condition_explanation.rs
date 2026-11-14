use nodex_ir::{Concept, reading::ReadingContent};
use nodex_lang_rust::lower_source;

#[test]
fn if_let_explains_match_scope_and_both_outcomes_without_guessing_some() {
    let source = "fn run(value: Option<i32>) { if let Some(x) = value { consume(x); } else { miss(); } if let Some(y) = value { consume(y); } while let Some(z) = value { consume(z); } if ready() { miss(); } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "run").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Branch by matching a pattern")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in sites {
        let node = flow.nodes.iter().find(|n| n.id == site.node).unwrap();
        assert_eq!(node.concept, Concept::Branch);
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "if");
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        assert!(text.contains("Evaluate value once"));
        assert!(text.contains("names are available in that body"));
        assert!(text.contains("else branch if one is present"));
        assert!(text.contains("move, copy, or borrow"));
        assert!(text.contains("does not establish"));
    }
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Pattern { route, .. } if route.as_deref() == Some("true"))));
}
