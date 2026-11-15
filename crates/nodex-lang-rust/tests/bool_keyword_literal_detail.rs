use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_boolean_literals_without_matching_string_text() {
    let source = "const FLAG: bool = true; fn f() { let x = true; let y = false; let s = \"true false\"; }";
    let file = lower_source("src/lib.rs", source);
    let flag = file.symbols.iter().find(|symbol| symbol.name == "FLAG").unwrap();
    assert!(flag.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Boolean literal keyword" && item.text.contains("type `bool`")));
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Boolean literal keyword")).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for (item, word) in items.iter().zip(["true", "false"]) {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], word);
        assert!(matches!(&item.content, ReadingContent::Detail { text, .. } if text.contains("exactly two values") && text.contains("pattern") && text.contains("not a string")));
    }
}
