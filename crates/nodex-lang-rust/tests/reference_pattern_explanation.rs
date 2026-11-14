use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_reference_pattern_without_confusing_ref_or_borrow_expression() {
    let source = "fn f(input: &i32) { let &x = input; let borrow = &x; match input { &y => consume(y) } let ref z = x; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Match through a reference")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    assert!(facts.iter().all(|site| matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("reference layer already present") && text.contains("does not create a new reference")
        && text.contains("ref x") && text.contains("expression creates a borrow"))));
}
