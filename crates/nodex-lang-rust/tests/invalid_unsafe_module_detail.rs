use nodex_lang_rust::lower_source;

#[test]
fn rejects_unsafe_module_without_confusing_unsafe_block() {
    let source = "unsafe mod inner {} mod valid {} fn f() { unsafe { work(); } }";
    let ir = lower_source("src/lib.rs", source);
    let module = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    let item = module.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Invalid unsafe module declaration").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "unsafe");
    for phrase in ["not a valid Rust module", "cannot qualify `mod`", "does not establish an unsafe context", "remove `unsafe`", "parser recovered", "rather than claiming"] { assert!(item.text.contains(phrase), "missing {phrase}"); }
    for name in ["valid", "f"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Invalid unsafe module declaration"));
    }
}
