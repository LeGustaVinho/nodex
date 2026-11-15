use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn mutable_reference_pattern_and_borrow_are_both_explained() {
    let source = "fn f() { let mut value = 1; let &mut x = &mut value; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().collect();
    assert!(items.iter().any(|s| matches!(&s.content, ReadingContent::Detail { label, text, .. }
        if label == "Match through a mutable reference" && text.contains("binding by value") && text.contains("ref mut x"))));
    assert!(items.iter().any(|s| matches!(&s.content, ReadingContent::Detail { label, text, .. }
        if label == "Borrow with exclusive mutable access" && text.contains("&mut value") && text.contains("exclusive borrow"))));
}
