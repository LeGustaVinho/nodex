#[test]
fn explains_function_declaration_even_with_empty_body() {
    let source = "fn foo() {}";
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "foo").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter()
        .find(|item| item.group == "Function").unwrap();
    assert!(item.text.contains("fn declares a function named foo"));
    assert!(item.text.contains("body runs when the function is called"));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "fn");
}
