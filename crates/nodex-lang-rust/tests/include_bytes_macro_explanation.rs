use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn include_bytes_is_conditional_and_visible_in_flow_and_constant() {
    let source = "const DATA: &[u8] = include_bytes!(\"file.bin\"); fn f() { let b = include_bytes!(\"file.bin\"); let c = other::include_bytes!(\"shadow.bin\"); let text = include_str!(\"x\"); let other = include_bytes!(name); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Embed file bytes at compile time")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    assert!(facts.iter().all(|site| matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("If `") && text.contains("relative to the source file") && text.contains("&'static [u8; N]") && text.contains("does not read the file when the program later runs") && text.contains("does not prove"))));
    let data = ir.symbols.iter().find(|s| s.name == "DATA").unwrap();
    assert!(data.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Embed file bytes at compile time" && item.text.contains("file.bin")));
}
