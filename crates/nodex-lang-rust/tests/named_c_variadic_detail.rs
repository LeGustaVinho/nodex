use nodex_lang_rust::lower_source;

#[test]
fn distinguishes_named_variadic_declaration_from_fixed_parameter() {
    let source = "unsafe extern \"C\" { fn f(args: ...); fn g(x: i32, tail: ...); fn plain(x: i32, ...); fn fixed(args: i32); } unsafe extern \"system\" { fn system(args: ...); }";
    let ir = lower_source("src/lib.rs", source);
    for (name, arg) in [("f", "args"), ("g", "tail")] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Named foreign C variadic parameter").unwrap();
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], arg);
        for phrase in ["name is optional", "not a fixed argument", "zero or more extra arguments", "argument promotions", "undefined behavior", "does not verify"] { assert!(item.text.contains(phrase), "{name}: {phrase}"); }
        assert!(symbol.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Variadic foreign C function"));
    }
    for name in ["plain", "fixed", "system"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Named foreign C variadic parameter"), "{name}");
    }
}
