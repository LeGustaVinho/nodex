use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_for_iteration_on_the_loop_node_without_guessing_item_ownership() {
    let source = "fn f(xs: Vec<i32>) { for x in xs { consume(x); } while ready() { tick(); } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let found: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Repeat for each iterator item"
    )).collect();
    assert_eq!(found.len(), 1, "{:?}", flow.reading);
    let detail = found[0];
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "for");
    assert!(matches!(&detail.content, ReadingContent::Detail { text, .. }
        if text.contains("Evaluate xs once") && text.contains("pattern x") && text.contains("no next item") && text.contains("syntax alone does not establish")));
}
