use nodex_lang_rust::lower_source;

#[test]
fn explains_mutable_static_and_not_ordinary_static() {
    let source = "static mut X: i32 = 10; static Y: i32 = 20; fn f() { let mut z = 1; z += 1; }";
    let ir = lower_source("src/lib.rs", source);
    let x = ir.symbols.iter().find(|symbol| symbol.name == "X").unwrap();
    let item = x.rust_contract.as_ref().unwrap().iter()
        .find(|item| item.group == "Mutable static").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "mut");
    assert!(item.text.contains("entire program") && item.text.contains("unsafe") && item.text.contains("data races"));
    let y = ir.symbols.iter().find(|symbol| symbol.name == "Y").unwrap();
    assert!(!y.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Mutable static")));
}
