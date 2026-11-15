use nodex_lang_rust::lower_source;

#[test]
fn no_std_is_explained_on_the_file_module_without_assuming_std_is_forbidden() {
    let source = "#![no_std] extern crate alloc; extern crate std; fn f() {} mod nested { #![allow(dead_code)] fn g() {} }";
    let ir = lower_source("src/lib.rs", source);
    let module = ir.symbols.iter().find(|symbol| symbol.parent.is_none()).unwrap();
    let item = module.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Crate without automatic std").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#![no_std]");
    assert!(item.text.contains("prevents automatic linking") && item.text.contains("core") && item.text.contains("alloc") && item.text.contains("does not forbid `std`") && item.text.contains("crate root"));
    let nested = ir.symbols.iter().find(|symbol| symbol.name == "nested").unwrap();
    assert!(!nested.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Crate without automatic std"));
    let ordinary = lower_source("src/lib.rs", "fn f() {}");
    let root = ordinary.symbols.iter().find(|symbol| symbol.parent.is_none()).unwrap();
    assert!(!root.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Crate without automatic std"));
}
