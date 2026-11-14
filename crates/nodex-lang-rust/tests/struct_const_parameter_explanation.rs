use nodex_lang_rust::lower_source;

#[test]
fn struct_const_parameter_is_explained_on_its_symbol() {
    let text = "struct Buffer<const N: usize> { bytes: [u8; N] } struct Plain<T>(T); fn f<const N: usize>() {}";
    let ir = lower_source("src/lib.rs", text);
    let buffer = ir.symbols.iter().find(|symbol| symbol.name == "Buffer").unwrap();
    let item = buffer.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Const generic parameter").unwrap();
    assert_eq!(&text[item.span.start_byte as usize..item.span.end_byte as usize], "const N: usize");
    assert!(item.text.contains("compile-time value"));
    assert!(item.text.contains("different instantiations"));
    assert!(item.text.contains("does not declare a field"));
    let plain = ir.symbols.iter().find(|symbol| symbol.name == "Plain").unwrap();
    assert!(!plain.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Const generic parameter"));
}
