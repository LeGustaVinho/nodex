use nodex_lang_rust::lower_source;

#[test]
fn explains_arc_typed_receiver_without_assuming_name_resolution() {
    let text = "struct S; impl S { fn receive(self: Arc<Self>) {} fn normal(&self) {} fn other(self: Box<Self>) {} }";
    let ir = lower_source("src/lib.rs", text);
    let method = ir.symbols.iter().find(|s| s.name == "receive").unwrap();
    assert!(!method.flavors.iter().any(|flavor| flavor == "static"));
    let item = method.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Typed receiver").unwrap();
    assert!(item.text.contains("takes that receiver by value"));
    assert!(item.text.contains("If `Arc` resolves"));
    assert!(item.text.contains("reference count"));
    assert!(item.text.contains("does not grant unrestricted mutation"));
    assert_eq!(&text[item.span.start_byte as usize..item.span.end_byte as usize], "self: Arc<Self>");
    for name in ["normal", "other"] {
        let method = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!method.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Typed receiver" && item.text.contains("reference-counted")));
    }
}
