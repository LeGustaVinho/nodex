use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn never_type_explained_in_return_alias_and_local_annotation() {
    // Some type positions are feature-gated by Rust; the syntax analysis is independent of compilation.
    let text = "type NoValue = Result<i32, !>; fn stop() -> ! { loop {} } fn local() { let value: ! = stop(); } fn ordinary() -> () {}";
    let ir = lower_source("src/lib.rs", text);
    for name in ["NoValue", "stop"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let detail = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Never type").expect(name);
        assert!(detail.text.contains("no values"));
        assert!(detail.text.contains("loop forever"));
        assert_eq!(&text[detail.span.start_byte as usize..detail.span.end_byte as usize], "!");
    }
    let ordinary = ir.symbols.iter().find(|symbol| symbol.name == "ordinary").unwrap();
    assert!(!ordinary.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Never type"));
    let flow = ir.flows.iter().find(|flow| flow.name == "local").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Never type" && text.contains("no values"))), "{:?}", flow.reading);
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
}

