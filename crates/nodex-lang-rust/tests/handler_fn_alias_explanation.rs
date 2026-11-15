use nodex_lang_rust::lower_source;

#[test]
fn alias_explains_function_pointer_signature_and_values() {
    let source = "type Handler = fn(i32) -> bool; type Other = fn() -> (); type UnsafeHandler = unsafe fn(i32) -> bool; type Foreign = extern \"C\" fn(i32) -> bool; type Number = i32;";
    let ir = lower_source("src/lib.rs", source);
    for (name, params, result) in [("Handler", "(i32)", "bool"), ("Other", "()", "()")] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Function pointer alias").unwrap();
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "fn");
        for phrase in ["another name", "neither a new distinct type nor a function", "compatible function", "non-capturing closure", "capturing its environment cannot", "does not select or call"] { assert!(item.text.contains(phrase), "{name}: {}", item.text); }
        assert!(item.text.contains(&format!("argument types {params}")) && item.text.contains(&format!("returns {result}")));
    }
    for name in ["UnsafeHandler", "Foreign", "Number"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Function pointer alias"), "{name}");
    }
}
