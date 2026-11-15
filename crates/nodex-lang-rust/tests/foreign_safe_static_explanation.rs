use nodex_lang_rust::lower_source;

#[test]
fn safe_foreign_static_is_explained_on_owning_module_without_guessing_other_statics() {
    let source = "unsafe extern \"C\" { safe static X: i32; pub safe static PUBLIC: u8; static Z: u32; safe static mut MUT: i32; // safe\n static COMMENT: i32; } mod nested { unsafe extern \"C\" { safe static N: i32; } } mod other { unsafe extern \"system\" { safe static OTHER: i32; } }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.parent.is_none()).unwrap();
    let nested = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.name == "nested").unwrap();
    for (symbol, expected) in [(root, vec!["X", "PUBLIC"]), (nested, vec!["N"])] {
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Safe foreign static").collect();
        assert_eq!(facts.len(), expected.len(), "{}: {:?}", symbol.name, symbol.rust_contract);
        for name in expected {
            let item = facts.iter().find(|item| item.text.contains(&format!("declares {name} as a foreign static"))).unwrap();
            assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "safe");
            assert!(item.text.contains("without an unsafe block"));
            assert!(item.text.contains("linker must resolve"));
            assert!(item.text.contains("before Rust code runs"));
            assert!(item.text.contains("does not prove"));
            assert!(item.text.contains("mutable static has separate unsafe access rules"));
        }
    }
    let other = ir.symbols.iter().find(|s| s.name == "other").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Safe foreign static"));
}
