use nodex_lang_rust::lower_source;

#[test]
fn explains_precise_opaque_capture_without_claiming_runtime_capture() {
    let source = "trait Trait {} fn f<'a, T>(x: &'a T) -> impl Trait + use<'a, T> { loop {} } fn empty() -> impl Trait + use<> { loop {} } fn normal<T>() -> impl Trait { loop {} }";
    let ir = lower_source("src/lib.rs", source);
    let f = ir.symbols.iter().find(|s| s.name == "f").unwrap();
    let item = f.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Precise opaque type capture").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "use<'a, T>");
    assert!(item.text.contains("'a, T"));
    assert!(item.text.contains("type-level permission"));
    assert!(item.text.contains("not a runtime copy or move"));
    let empty = ir.symbols.iter().find(|s| s.name == "empty").unwrap();
    assert!(empty.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Precise opaque type capture" && item.text.contains("list is empty")));
    let normal = ir.symbols.iter().find(|s| s.name == "normal").unwrap();
    assert!(!normal.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Precise opaque type capture"));
}
