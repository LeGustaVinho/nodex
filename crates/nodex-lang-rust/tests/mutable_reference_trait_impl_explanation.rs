use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn mutable_reference_trait_impl_is_explained_on_module_even_when_empty() {
    let source = "trait Trait {} impl<T> Trait for &mut T {} impl<T> Trait for &T {} impl Trait for &mut Concrete {} impl<T> Trait for T {} mod inner { impl<'a, U: Bound> Trait for &'a mut U { fn f(&self) {} } }";
    let file = lower_source("src/lib.rs", source);
    let root = file.symbols.iter().find(|s| s.kind == SymbolKind::Module && s.parent.is_none()).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Trait implementation for mutable references").collect();
    assert_eq!(items.len(), 1, "{:?}", root.rust_contract);
    assert!(source[items[0].span.start_byte as usize..items[0].span.end_byte as usize].contains("impl<T> Trait for &mut T"));
    assert!(items[0].text.contains("not by itself implement the trait for T itself"));
    assert!(items[0].text.contains("exclusive borrow"));
    assert!(items[0].text.contains("does not imply that every call mutates"));
    let inner = file.symbols.iter().find(|s| s.kind == SymbolKind::Module && s.name == "inner").unwrap();
    let nested: Vec<_> = inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Trait implementation for mutable references").collect();
    assert_eq!(nested.len(), 1, "{:?}", inner.rust_contract);
    assert!(nested[0].text.contains("&'a mut U"));
    let method = file.symbols.iter().find(|s| s.name == "f").unwrap();
    assert!(method.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Trait implementation for mutable references"));
}
