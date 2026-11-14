#[test]
fn explains_named_struct_and_its_fields() {
    let source = "struct Point { x: f32, y: f32 }";
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "Point").unwrap();
    let explanation = symbol.rust_contract.as_ref().unwrap().iter()
        .find(|item| item.group == "Structure").unwrap();
    assert!(explanation.text.contains("struct defines Point as a new type"));
    assert!(explanation.text.contains("x: f32, y: f32"));
    assert!(explanation.text.contains("32-bit floating-point"));
    assert!(explanation.text.contains("does not create a value"));
    assert_eq!(&source[explanation.span.start_byte as usize..explanation.span.end_byte as usize], source);
}
