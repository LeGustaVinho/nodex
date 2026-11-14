use nodex_lang_rust::lower_source;

#[test]
fn explains_inline_clone_bound_without_claiming_automatic_copy() {
    let source = "fn f<T: Clone>(x: T) {} fn no<T>(x: T) {} fn different<T: Copy>(x: T) {} trait Service { fn go<T: Clone>(&self, x: T); }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "go"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Generic Clone bound").unwrap();
        assert!(item.text.contains("same chosen type"));
        assert!(item.text.contains("If Clone resolves"));
        assert!(item.text.contains("not automatic"));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "Clone");
    }
    for name in ["no", "different"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Generic Clone bound"));
    }
}
