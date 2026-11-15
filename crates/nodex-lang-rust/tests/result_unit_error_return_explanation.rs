use nodex_lang_rust::lower_source;

#[test]
fn explains_unit_success_and_error_conditionally_in_function_returns() {
    let source = "fn f() -> Result<(), Error> { todo!() } trait T { fn run(&self) -> Result<(), Error>; } fn value() -> Result<i32, Error> { todo!() } fn option() -> Option<()> { None }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "run"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let detail = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Result with unit success").unwrap();
        assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "Result<(), Error>");
        assert!(detail.text.contains("If `Result` resolves") && detail.text.contains("Ok(())") && detail.text.contains("Err(error)") && detail.text.contains("ordinary return") && detail.text.contains("do not establish"));
    }
    for name in ["value", "option"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Result with unit success"), "{name}");
    }
}
