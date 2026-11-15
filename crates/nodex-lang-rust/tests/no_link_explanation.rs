use nodex_lang_rust::lower_source;
#[test]
fn no_link_is_explained_on_the_enclosing_module_for_direct_extern_crates_only() {
    let source = "#[no_link] // first\n#[no_link] #[allow(unused)] extern crate foo as bar; extern crate plain; mod inner { #[no_link] extern crate nested; #[cfg_attr(test, no_link)] extern crate conditional; } #[no_link] fn wrong_place() {} #[other(no_link)] extern crate not_direct;";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.id == "ns:crate::lib").or_else(|| ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.parent.is_none())).unwrap();
    let root_items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "External crate without linking").collect();
    assert_eq!(root_items.len(), 1, "{:?}", root.rust_contract);
    let first = &root_items[0];
    assert_eq!(first.span.start_byte as usize, source.find("#[no_link]").unwrap());
    assert_eq!(&source[first.span.start_byte as usize..first.span.end_byte as usize], "#[no_link]");
    assert!(first.text.contains("extern crate foo") && first.text.contains("not to link") && first.text.contains("macros") && first.text.contains("link errors"));
    let inner = ir.symbols.iter().find(|s| s.name == "inner").unwrap();
    let nested: Vec<_> = inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "External crate without linking").collect();
    assert_eq!(nested.len(), 1, "{:?}", inner.rust_contract);
    assert!(nested[0].text.contains("extern crate nested"));
    let wrong = ir.symbols.iter().find(|s| s.name == "wrong_place").unwrap();
    assert!(!wrong.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "External crate without linking"));
}
