use nodex_lang_rust::lower_source;

#[test]
fn foreign_safe_function_retains_qualifier_and_explains_calling_contract() {
    let source = "unsafe extern \"C\" { safe fn f(); pub safe fn public_f(x: i32) -> i32; fn implicit_unsafe(); unsafe fn explicit_unsafe(); } fn ordinary() {}";
    let file = lower_source("src/lib.rs", source);
    for (name, expected) in [("f", "safe fn f()"), ("public_f", "pub safe fn public_f(x: i32) -> i32")] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert_eq!(symbol.signature.as_deref(), Some(expected));
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Explicitly safe foreign function").expect("safe foreign contract");
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "safe");
        assert!(item.text.contains("safe Rust may call"));
        assert!(item.text.contains("does not verify the foreign implementation"));
        assert!(item.text.contains("extern \"C\""));
    }
    for name in ["implicit_unsafe", "explicit_unsafe", "ordinary"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Explicitly safe foreign function"));
    }
}
