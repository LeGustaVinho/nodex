use nodex_lang_rust::lower_source;

#[test]
fn unsafe_impl_explained_on_type_even_without_members() {
    let source = "unsafe trait Trusted {} struct S; unsafe impl Trusted for S {} struct Plain; impl Trusted for Plain {}";
    let ir = lower_source("src/lib.rs", source);
    let s = ir.symbols.iter().find(|symbol| symbol.name == "S").unwrap();
    let detail = s.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Unsafe trait implementation").unwrap();
    assert!(detail.text.contains("safety requirements"));
    assert!(detail.text.contains("compiler does not prove"));
    assert!(detail.text.contains("empty braces"));
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "unsafe");
    let plain = ir.symbols.iter().find(|symbol| symbol.name == "Plain").unwrap();
    assert!(!plain.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Unsafe trait implementation")));
}
