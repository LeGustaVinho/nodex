use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn unknown_question_mark_explains_both_possibilities_without_guessing() {
    let source = "fn f(x: Option<i32>) -> Option<i32> { let value = x?; Some(value) }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let detail = flow.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Continue or propagate with ?")).unwrap_or_else(|| panic!("{:?}", flow.reading));
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "?");
    assert!(matches!(&detail.content, ReadingContent::Detail { text, .. } if text.contains("type is not established") && text.contains("If it is the standard Result") && text.contains("If it is the standard Option") && text.contains("does not catch panics")));
    assert!(flow.reading.iter().flatten().any(|site| site.span == detail.span && matches!(&site.content, ReadingContent::Paths { .. })));
}
