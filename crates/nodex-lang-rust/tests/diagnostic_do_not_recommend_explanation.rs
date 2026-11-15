use nodex_lang_rust::lower_source;
#[test]
fn diagnostic_hint_is_explained_for_trait_impls_even_when_empty_or_blanket() {
    let source = "trait Mark { fn act(&self); } trait Other {} trait Blanket {} struct S; #[diagnostic::do_not_recommend] // note\n#[allow(dead_code)] impl Mark for S { fn act(&self) {} } #[diagnostic::do_not_recommend] impl Other for S {} #[diagnostic::do_not_recommend] impl<T> Blanket for T {} #[diagnostic::do_not_recommend] impl S { fn inherent(&self) {} } #[diagnostic::do_not_recommend] fn unrelated() {}";
    let ir = lower_source("src/lib.rs", source);
    let module = ir.symbols.iter().find(|s| s.id == "ns:crate::lib").or_else(|| ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module)).unwrap();
    let module_items: Vec<_> = module.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Diagnostic recommendation hint").collect();
    assert_eq!(module_items.len(), 3, "{:?}", module.rust_contract);
    for item in module_items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[diagnostic::do_not_recommend]");
        assert!(item.text.contains("hint") && item.text.contains("not to show") && item.text.contains("does not remove") && item.text.contains("trait resolution"));
    }
    let target = ir.symbols.iter().find(|s| s.name == "S").unwrap();
    assert_eq!(target.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Diagnostic recommendation hint").count(), 2);
    let method = ir.symbols.iter().find(|s| s.name == "act" && s.parent.as_deref().is_some_and(|p| p.ends_with("::S"))).unwrap();
    assert!(method.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Diagnostic recommendation hint"));
    let unrelated = ir.symbols.iter().find(|s| s.name == "unrelated").unwrap();
    assert!(!unrelated.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Diagnostic recommendation hint"));
}
