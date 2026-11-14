use nodex_ir::reading::ReadingContent;

#[test]
fn exposes_conditional_tuple_variant_construction_in_flow() {
    let source = "enum Message { Quit, Move(i32, i32) } fn f() { let value = Message::Move(1, 2); }";
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, text, .. }
        if label == "Construct a tuple-style enum variant" && text.contains("If Message::Move resolves") && text.contains("same position"))));
}
