use nodex_lang_rust::lower_source;

#[test]
fn macro_rules_definition_is_explained_on_its_symbol() {
    let source = "macro_rules! print_one { ($x:expr) => { println!(\"{{}}\", $x) }; () => { println!(\"empty\") }; } fn ordinary() {} fn use_it() { print_one!(3); }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|s| s.name == "print_one!").expect("macro symbol");
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Declarative macro definition").expect("macro explanation");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "print_one");
    assert!(item.text.contains("first matching rule") && item.text.contains("input tokens") && item.text.contains("not ordinary function arguments"));
    let function = ir.symbols.iter().find(|s| s.name == "ordinary").unwrap();
    assert!(!function.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Declarative macro definition"));
}
