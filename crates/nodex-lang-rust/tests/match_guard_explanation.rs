use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn guarded_arm_explains_order_and_fallthrough_in_the_flow() {
    let source = "fn check(x: Option<i32>) -> i32 { match x { Some(n) if n > 0 => n, _ => 0 } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "check").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Check a match guard")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let site = sites[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "if");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("Only if it matches") && text.contains("n > 0")
        && text.contains("If the guard is true") && text.contains("later arm")));
}
