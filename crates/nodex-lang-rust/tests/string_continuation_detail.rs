use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_continuation_without_misreading_escaped_backslash_or_raw_string() {
    let source = concat!("fn f() { let s = \"first", "\\", "\n", "   second\"; let escaped = \"a", "\\\\", "\n", "b\"; let raw = r\"first", "\\", "\n", " second\"; }");
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "String line continuation")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    let site = details[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "\\\n   ");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("no characters") && text.contains("additional line breaks") && text.contains("firstsecond") && text.contains("raw strings")));
}
