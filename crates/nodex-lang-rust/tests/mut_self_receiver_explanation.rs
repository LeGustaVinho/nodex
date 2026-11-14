#[test]
fn distinguishes_mutable_by_value_receiver_from_borrowed_receivers() {
    let source = "struct S; impl S { fn consume(mut self) {} fn borrowed(&mut self) {} fn plain(self) {} }";
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let receiver = |name: &str| {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        symbol.rust_contract.as_ref().unwrap().iter()
            .find(|item| item.group == "Receiver").unwrap().clone()
    };
    let owned = receiver("consume");
    assert!(owned.text.contains("receives the value by value"));
    assert!(owned.text.contains("changes to the local receiver binding"));
    assert!(owned.text.contains("moved or copied according to its type"));
    assert_eq!(&source[owned.span.start_byte as usize..owned.span.end_byte as usize], "mut self");
    assert!(receiver("borrowed").text.contains("borrows that value exclusively"));
    assert!(!receiver("plain").text.contains("changes to the local receiver binding"));
}
