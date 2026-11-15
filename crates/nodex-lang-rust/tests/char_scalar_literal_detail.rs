use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn distinguishes_scalars_from_byte_and_string_literals() {
    let source = "const A: char = 'a'; fn f() { let a = 'a'; let emoji = '😀'; let escaped = '\\n'; let byte = b'a'; let string = \"a\"; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Unicode scalar character literal")).collect();
    assert_eq!(facts.len(), 3, "{:?}", flow.reading);
    for fact in facts { assert!(matches!(&fact.content, ReadingContent::Detail { text, .. } if text.contains("one Unicode scalar") && text.contains("surrogate") && text.contains("multiple scalars") && text.contains("byte literal") && text.contains("string literal"))); }
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "A").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Unicode scalar character literal").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "'a'");
}
