use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn compile_error_is_explained_conditionally_on_flow_and_owning_symbol() {
    let source = r#"compile_error!("root"); mod inner { #[cfg(any())] compile_error!("excluded"); } const C: () = compile_error!("constant"); fn f(flag: bool) { if flag { compile_error!("message"); } custom::compile_error!("custom"); compile_error!(NAME); other!("message"); }"#;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Emit a compile-time error")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in &sites {
        assert!(source[site.span.start_byte as usize..site.span.end_byte as usize].starts_with('"'));
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("If ") && text.contains("standard compile_error macro") && text.contains("conditional compilation")
                && text.contains("prevents a successful compilation") && text.contains("not a runtime print")
                && text.contains("does not establish") && text.contains("may behave differently")));
    }
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("custom::compile_error"))));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Macro invocation")), "{:?}", flow.reading);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none()).unwrap();
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "C").unwrap();
    for (symbol, expected) in [(root, "root"), (inner, "excluded"), (constant, "constant")] {
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Compile-time error macro").collect();
        assert_eq!(items.len(), 1, "{}: {:?}", symbol.name, symbol.rust_contract);
        assert!(items[0].text.contains(expected) && items[0].text.contains("conditional compilation"));
    }
}
