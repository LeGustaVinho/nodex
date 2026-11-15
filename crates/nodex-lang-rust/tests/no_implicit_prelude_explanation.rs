use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn explains_inner_no_implicit_prelude_at_crate_and_module_scope() {
    let source = "#![no_implicit_prelude]\nmod inner { #![no_implicit_prelude] fn f() {} } mod plain { #![allow(dead_code)] fn g() {} }";
    let file = lower_source("src/lib.rs", source);
    let root = file.symbols.iter().find(|symbol| symbol.kind == SymbolKind::Module && symbol.parent.is_none()).unwrap();
    let inner = file.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    for symbol in [root, inner] {
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Disable implicit prelude lookups").collect();
        assert_eq!(items.len(), 1, "{symbol:?}");
        assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "#![no_implicit_prelude]");
        assert!(items[0].text.contains("standard-library prelude"));
        assert!(items[0].text.contains("extern prelude"));
        assert!(items[0].text.contains("macro_use prelude"));
        assert!(items[0].text.contains("tool prelude"));
        assert!(items[0].text.contains("does not by itself remove the standard library"));
    }
    let plain = file.symbols.iter().find(|symbol| symbol.name == "plain").unwrap();
    assert!(!plain.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Disable implicit prelude lookups"));
}
