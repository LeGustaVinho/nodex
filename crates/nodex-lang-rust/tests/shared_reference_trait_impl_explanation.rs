use nodex_lang_rust::lower_source;
#[test]
fn generic_shared_reference_trait_impl_is_visible_on_its_module() {
    let source = "trait Trait {} trait Bound {} impl<T> Trait for &T {} impl<T: Bound> Trait for &mut T {} impl<T> Trait for T {} impl Trait for &i32 {} mod inner { impl<'a, U: Bound> Trait for &'a U {} }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.parent.is_none()).unwrap();
    let inner = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.name == "inner").unwrap();
    for (symbol, target) in [(root, "&T"), (inner, "&'a U")] {
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Trait implementation for a shared reference").collect();
        assert_eq!(items.len(), 1, "{}: {:?}", symbol.name, symbol.rust_contract);
        let item = items[0];
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].contains(target));
        assert!(item.text.contains("shared-reference type") && item.text.contains("declared bounds") && item.text.contains("rather than necessarily for the owned value") && item.text.contains("interior-mutability") && item.text.contains("checks coherence"));
    }
}
