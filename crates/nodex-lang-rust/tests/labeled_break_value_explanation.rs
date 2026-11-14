use nodex_ir::{Concept, reading::ReadingContent};
use nodex_lang_rust::lower_source;

#[test]
fn labeled_break_values_are_explained_on_the_exit_node() {
    let text = "fn f() { let answer = 'outer: loop { loop { break 'outer 42; } }; let other = 'block: { break 'block 7; }; 'no_value: loop { break 'no_value; } loop { break 3; } }";
    let ir = lower_source("src/lib.rs", text);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Leave a named destination with a value")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(flow.node(&site.node).unwrap().concept, Concept::Break);
        let ReadingContent::Detail { text: explanation, .. } = &site.content else { unreachable!() };
        assert!(explanation.contains("becomes the result") && explanation.contains("while") && explanation.contains("for"));
        let span = &text[site.span.start_byte as usize..site.span.end_byte as usize];
        assert!(span == "'outer" || span == "'block");
    }
}
