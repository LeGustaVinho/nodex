use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_a_value_break_to_a_labeled_block_without_confusing_loop_breaks() {
    let source = "fn f() -> i32 { let result = 'block: { if ready() { break 'block 42; } 7 }; 'repeat: loop { break 'repeat result; } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Leave a labeled block with a value")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "break");
    assert!(matches!(&facts[0].content, ReadingContent::Detail { text, .. }
        if text.contains("Evaluate 42") && text.contains("value of that block expression") && text.contains("does not repeat")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Leave a named destination with a value")
        && &source[site.span.start_byte as usize..site.span.end_byte as usize] == "'block"));
}
