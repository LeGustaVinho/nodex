use nodex_lang_rust::lower_source;

#[test]
fn explains_consuming_receiver_in_the_method_contract() {
    let ir = lower_source("src/lib.rs", "struct S; impl S { fn consume(self) {} fn borrow(&self) {} }");
    let contract = ir.symbols.iter().find(|symbol| symbol.name == "consume").unwrap().rust_contract.as_ref().unwrap();
    let text = &contract.iter().find(|item| item.group == "Receiver").unwrap().text;
    assert!(text.contains("self receives the value by value"));
    assert!(text.contains("implements Copy"));
    assert!(text.contains("caller retains the original"));
    assert!(text.contains("value is moved into the method"));
    assert!(text.contains("caller cannot use that moved value afterward"));
    let borrow = ir.symbols.iter().find(|symbol| symbol.name == "borrow").unwrap().rust_contract.as_ref().unwrap();
    assert!(!borrow.iter().any(|item| item.group == "Receiver" && item.text.contains("value is moved into the method")));
}
