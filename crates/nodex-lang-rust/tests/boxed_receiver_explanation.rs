use nodex_lang_rust::lower_source;

#[test]
fn boxed_receiver_is_explained_without_assuming_name_resolution() {
    let source = "struct S; impl S { fn take(self: Box<Self>) {} fn other(x: Box<Self>) {} fn shared(&self) {} } trait T { fn take(self: std::boxed::Box<Self>); }";
    let ir = lower_source("src/lib.rs", source);
    let methods: Vec<_> = ir.symbols.iter().filter(|symbol| symbol.name == "take").collect();
    assert_eq!(methods.len(), 2);
    for method in methods {
        assert!(!method.flavors.iter().any(|flavor| flavor == "static"));
        let item = method.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Boxed receiver").expect("boxed receiver contract");
        assert!(item.text.contains("If `"));
        assert!(item.text.contains("standard Box"));
        assert!(item.text.contains("caller cannot use"));
        assert!(item.text.contains("cannot prove"));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("self:"));
    }
    for method in ir.symbols.iter().filter(|symbol| symbol.name == "other" || symbol.name == "shared") {
        if method.name == "other" { assert!(method.flavors.iter().any(|flavor| flavor == "static")); }
        assert!(!method.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Boxed receiver"));
    }
}
