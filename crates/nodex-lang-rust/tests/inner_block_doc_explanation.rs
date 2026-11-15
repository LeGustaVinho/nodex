use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn explains_inner_block_documentation_on_its_own_module() {
    let source = "/*! Root documentation */\nmod inner { /*! Inner documentation */ fn f() {} }\nmod ordinary { /* ordinary */ fn g() {} }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    let ordinary = ir.symbols.iter().find(|symbol| symbol.name == "ordinary").unwrap();
    for (symbol, expected) in [(root, "/*! Root documentation */"), (inner, "/*! Inner documentation */")] {
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Inner block documentation").collect();
        assert_eq!(items.len(), 1, "{symbol:?}");
        assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], expected);
        assert!(items[0].text.contains("enclosing crate, module or item"));
        assert!(items[0].text.contains("not an instruction executed"));
    }
    assert!(!ordinary.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Inner block documentation"));
}
