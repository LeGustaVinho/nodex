use nodex_lang_rust::lower_source;

#[test]
fn async_function_contract_distinguishes_future_from_its_output() {
    let source = "async fn f(x: u32) -> u32 { x } async fn no_result() {} fn ordinary() -> u32 { 1 } trait T { async fn fetch(&self) -> u32; }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "no_result", "fetch"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let contract = symbol.rust_contract.as_ref().unwrap();
        let detail = contract.iter().find(|item| item.group == "Async function").unwrap();
        assert!(detail.text.contains("returns a future") && detail.text.contains("future's output") && detail.text.contains("does not schedule"));
        let returns = contract.iter().find(|item| item.group == "Returns").unwrap();
        assert!(returns.text.contains("returns a future"), "{}", returns.text);
        assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "async");
        if name != "fetch" {
            let function = contract.iter().find(|item| item.group == "Function").unwrap();
            assert!(function.text.contains("future is polled"));
            assert!(!function.text.contains("body runs when the function is called"));
        }
    }
    let ordinary = ir.symbols.iter().find(|s| s.name == "ordinary").unwrap();
    assert!(!ordinary.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Async function"));
    assert!(ordinary.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Function" && item.text.contains("body runs when the function is called")));
}
