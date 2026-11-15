use nodex_lang_rust::lower_source;

#[test]
fn explains_unnamed_constant_assertion_without_resolving_macro() {
    let source = "const _: () = { assert!(1 < 2); }; const OTHER: () = { assert!(true); }; const _: () = { custom::assert!(true); };";
    let ir = lower_source("src/lib.rs", source);
    let items: Vec<_> = ir.symbols.iter().flat_map(|symbol| symbol.rust_contract.as_ref().into_iter().flatten()).filter(|item| item.group == "Unnamed constant assertion").collect();
    assert_eq!(items.len(), 1);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "assert");
    for phrase in ["unnamed constant", "unit type", "compile-time", "If this invocation resolves", "false condition", "compilation fails", "not a runtime check", "resolve differently"] { assert!(items[0].text.contains(phrase), "missing {phrase}"); }
}
