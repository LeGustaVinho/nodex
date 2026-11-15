use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn foreign_static_is_explained_on_its_enclosing_module() {
    let source = r#"
        unsafe extern "C" { static X: i32; static mut Y: u8; safe static Z: i32; fn f(); }
        mod inner { unsafe extern "C" { static INNER: u32; } }
        static ORDINARY: i32 = 1;
    "#;
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Foreign static declaration").collect();
    assert_eq!(items.len(), 2, "{items:?}");
    for (written, variable_type) in [("static X: i32;", "i32"), ("static mut Y: u8;", "u8")] {
        let item = items.iter().find(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize] == written).unwrap();
        for phrase in [variable_type, "linked code", "unsafe to access", "unsafe context", "does not prove the symbol exists", "not a calling convention"] {
            assert!(item.text.contains(phrase), "{}", item.text);
        }
    }
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    let inner_items: Vec<_> = inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Foreign static declaration").collect();
    assert_eq!(inner_items.len(), 1);
    assert_eq!(&source[inner_items[0].span.start_byte as usize..inner_items[0].span.end_byte as usize], "static INNER: u32;");
}
