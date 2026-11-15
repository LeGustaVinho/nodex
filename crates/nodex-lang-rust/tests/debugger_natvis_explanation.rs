use nodex_lang_rust::lower_source;

#[test]
fn inner_natvis_attributes_preserve_paths_and_belong_to_their_module() {
    let source = r##"#![debugger_visualizer(natvis_file = "a.natvis")]
#![debugger_visualizer(natvis_file = "path with spaces/b.natvis")]
mod nested { #![debugger_visualizer(natvis_file = r#"raw path/c.natvis"#)] fn f() {} }
mod other { #![debugger_visualizer(gdb_script_file = "a.natvis")] }
#[cfg_attr(feature = "x", debugger_visualizer(natvis_file = "a.natvis"))] mod conditional {}"##;
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.parent.is_none()).unwrap();
    let nested = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.name == "nested").unwrap();
    for (symbol, expected) in [(root, vec!["\"a.natvis\"", "\"path with spaces/b.natvis\""]), (nested, vec!["r#\"raw path/c.natvis\"#"])] {
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Natvis debugger visualizer").collect();
        assert_eq!(facts.len(), expected.len(), "{}: {:?}", symbol.name, symbol.rust_contract);
        for written in expected {
            let item = facts.iter().find(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize] == written).unwrap();
            assert!(item.text.contains(written));
            assert!(item.text.contains("relative to the source file"));
            assert!(item.text.contains("-windows-msvc"));
            assert!(item.text.contains("does not establish that the file exists"));
            assert!(item.text.contains("does not execute the visualizer as Rust code"));
        }
    }
    for name in ["other", "conditional", "f"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Natvis debugger visualizer"), "{name}: {:?}", symbol.rust_contract);
    }
}
