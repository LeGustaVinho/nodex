#[test]
fn mutable_receiver_is_explained_in_method_and_trait_contracts() {
    let source = "struct Counter(i32); impl Counter { fn increment(&mut self) { self.0 += 1; } } trait Advance { fn advance(&mut self); }";
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    for name in ["increment", "advance"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).expect(name);
        let contract = symbol.rust_contract.as_ref().expect("method contract");
        let receiver = contract.iter().find(|item| item.group == "Receiver").expect("explanation of receiver");
        assert!(receiver.text.contains("self names the value"), "{}", receiver.text);
        assert!(receiver.text.contains("borrows that value exclusively"), "{}", receiver.text);
        assert!(receiver.text.contains("permits changes while the borrow lasts"), "{}", receiver.text);
        assert_eq!(&source[receiver.span.start_byte as usize..receiver.span.end_byte as usize], "&mut self");
    }
}
