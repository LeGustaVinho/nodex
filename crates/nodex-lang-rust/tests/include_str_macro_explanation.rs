use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn include_str_is_explained_conditionally_in_flow_and_constant_contract() {
    let source = r#"macro_rules! include_str { ($x:expr) => { "shadow" }; } const DATA: &str = include_str!("file.txt"); fn f() { let text = include_str!("file.txt"); let std_text = std::include_str!("file.txt"); let bytes = include_bytes!("file.txt"); let other = custom::include_str!("file.txt"); }"#;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Include a text file at compile time")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for fact in facts {
        assert!(matches!(&fact.content, ReadingContent::Detail { text, .. } if text.contains("If this invocation resolves") && text.contains("UTF-8") && text.contains("relative to the source file") && text.contains("&'static str") && text.contains("does not reopen") && text.contains("shadowed")));
    }
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "DATA").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Include a text file at compile time").expect("constant explanation");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "include_str");
    assert!(item.text.contains("If this invocation resolves"));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Invoke a macro with parentheses")), "generic macro detail should remain visible");
}
