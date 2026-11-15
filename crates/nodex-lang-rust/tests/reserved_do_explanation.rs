use nodex_lang_rust::lower_source;

#[test]
fn reserved_do_is_visible_in_invalid_code_without_matching_data_or_raw_identifiers() {
    let source = "fn invalid() { do { work(); } while condition; }";
    let valid_source = r##"fn raw() { let r#do = 1; let s = "do"; let r = r#"do"#; /* do */ // do
    let x = r#do; custom!(do); } fn ordinary() { let double = 1; }"##;
    let ir = lower_source("src/lib.rs", source);
    let invalid = ir.symbols.iter().find(|symbol| symbol.name == "invalid").unwrap();
    let facts: Vec<_> = invalid.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved keyword do").collect();
    assert_eq!(facts.len(), 1, "{:?}", invalid.rust_contract);
    let item = facts[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "do");
    for phrase in ["reserved Rust keyword", "does not provide", "do-while", "`loop` or `while`", "`r#do`", "syntax errors"] { assert!(item.text.contains(phrase), "{}", item.text); }
    let valid_ir = lower_source("src/lib.rs", valid_source);
    for name in ["raw", "ordinary"] {
        let symbol = valid_ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Reserved keyword do"), "{name}");
    }
}
