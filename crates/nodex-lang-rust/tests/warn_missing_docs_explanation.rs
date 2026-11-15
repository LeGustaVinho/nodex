use nodex_lang_rust::lower_source;

#[test]
fn explains_warning_level_without_claiming_diagnostics_occurred() {
    let source = "#[warn(missing_docs)] pub mod api { pub fn f() {} } #[warn(missing_docs)] pub fn exposed() {} #[allow(missing_docs)] pub fn allowed() {} #[warn(dead_code)] fn other() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["api", "exposed"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Warn on missing documentation").unwrap_or_else(|| panic!("{:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[warn(missing_docs)]");
        assert!(item.text.contains("warning level") && item.text.contains("public items") && item.text.contains("does not create documentation"));
        assert!(item.text.contains("compiler options") && item.text.contains("does not prove"));
    }
    for name in ["allowed", "other"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Warn on missing documentation"));
    }
}
