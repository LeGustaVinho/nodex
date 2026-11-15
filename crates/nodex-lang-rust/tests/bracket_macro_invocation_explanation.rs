use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_bracket_macro_without_conflating_other_delimiters_or_arrays() {
    let source = "fn f() { let a = name![value]; let b = name!(value); name! { value }; let array = [value]; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Macro invocation with brackets"
    )).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "[");
    assert!(matches!(&details[0].content, ReadingContent::Detail { text, .. }
        if text.contains("input tokens") && text.contains("not necessarily an array") && text.contains("does not establish")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Macro invocation"
    )), "{:?}", flow.reading);
}
