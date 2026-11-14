#[test]
fn explains_self_and_members_in_a_grouped_import() {
    let source = "use a::{self, b};";
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let module = ir.symbols.iter().find(|symbol| symbol.kind == nodex_ir::SymbolKind::Module).unwrap();
    let item = module.rust_contract.as_ref().unwrap().iter()
        .find(|item| item.group == "Grouped import").unwrap();
    assert!(item.text.contains("self imports the module a itself"));
    assert!(item.text.contains("(b)"));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], source);
}
