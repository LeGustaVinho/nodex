use nodex_lang_rust::lower_source;

#[test]
fn unsafe_functions_explain_caller_obligations_in_contracts() {
    let text = "unsafe fn read(p: *const i32) -> i32 { unsafe { *p } } trait T { unsafe fn required(&self); unsafe fn provided(&self) {} fn safe(&self) {} } fn safe() { unsafe { side_effect(); } } type Callback = unsafe fn();";
    let ir = lower_source("src/lib.rs", text);
    for name in ["read", "required", "provided"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let explanation = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Unsafe function").expect(name);
        assert!(explanation.text.contains("caller must uphold") && explanation.text.contains("documented safety contract"));
        assert!(explanation.text.contains("does not itself call") && explanation.text.contains("unsafe blocks"));
        assert_eq!(&text[explanation.span.start_byte as usize..explanation.span.end_byte as usize], "unsafe");
    }
    for symbol in ir.symbols.iter().filter(|symbol| symbol.name == "safe" || symbol.name == "Callback") {
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Unsafe function"));
    }
}
