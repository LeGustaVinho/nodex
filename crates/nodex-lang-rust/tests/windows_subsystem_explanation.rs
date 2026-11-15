use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn explains_windows_subsystem_conditionally_on_target_and_crate_type() {
    let source = "#![windows_subsystem = \"windows\"]\nmod inner {} fn main() {}";
    let file = lower_source("src/main.rs", source);
    let root = file.symbols.iter().find(|s| s.kind == SymbolKind::Module && s.parent.is_none()).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Windows GUI subsystem").collect();
    assert_eq!(items.len(), 1, "{:?}", root.rust_contract);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "#![windows_subsystem = \"windows\"]");
    assert!(items[0].text.contains("When active for a Windows binary"));
    assert!(items[0].text.contains("non-Windows targets and non-binary crate types"));
    assert!(items[0].text.contains("does not create a GUI"));
    for name in ["inner", "main"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Windows GUI subsystem"));
    }
    for source in ["#![windows_subsystem = \"console\"] fn main() {}", "#![cfg_attr(windows, windows_subsystem = \"windows\")] fn main() {}"] {
        let file = lower_source("src/main.rs", source);
        let root = file.symbols.iter().find(|s| s.kind == SymbolKind::Module && s.parent.is_none()).unwrap();
        assert!(!root.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Windows GUI subsystem"));
    }
}
