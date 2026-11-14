use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn match_arm_arrows_are_explained_on_the_match_node() {
    let source = "fn f(x: i32) -> i32 { match x { 0 => 1, n if n > 0 => n, _ => { 2 } } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let found: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "A match arm selects an expression"
    )).collect();
    assert_eq!(found.len(), 3, "{:?}", flow.reading);
    for site in found {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "=>");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("tries arms in order") && text.contains("guard succeeds") && text.contains("If that expression completes")));
    }
}
