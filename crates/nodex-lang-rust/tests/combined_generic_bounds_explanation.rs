use nodex_lang_rust::lower_source;

#[test]
fn explains_conjunction_on_generic_parameters_and_where_clauses() {
    let text = "trait A {} trait B {} fn inline<T: A + B>(x: T) {} fn constrained<T>(x: T) where T: A + B {} fn single<T: A>(x: T) {} fn relaxed<T: ?Sized + A>(x: &T) {}";
    let ir = lower_source("src/lib.rs", text);
    for name in ["inline", "constrained"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Combined generic bounds").expect(name);
        assert!(item.text.contains("every listed bound"));
        assert!(item.text.contains("compile-time requirements"));
        assert!(text[item.span.start_byte as usize..item.span.end_byte as usize].contains("A + B"));
    }
    for name in ["single", "relaxed"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Combined generic bounds"));
    }
}
