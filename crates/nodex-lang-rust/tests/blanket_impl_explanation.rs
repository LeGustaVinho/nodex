use nodex_lang_rust::lower_source;

#[test]
fn bounded_blanket_impl_without_members_is_visible_on_the_module() {
    let source = "trait Trait {} trait Other {} impl<T: Trait> Other for T {}";
    let ir = lower_source("src/lib.rs", source);
    let module = ir.symbols.iter().find(|symbol| symbol.id == "ns:crate::lib").or_else(|| ir.symbols.iter().find(|symbol| symbol.kind == nodex_ir::SymbolKind::Module)).expect("module symbol");
    let contract = module.rust_contract.as_ref().expect("module contract");
    assert!(contract.iter().any(|item| item.group == "Generic trait implementation" && item.text.contains("must satisfy Trait") && item.text.contains("can be empty")), "{contract:?}");
}
