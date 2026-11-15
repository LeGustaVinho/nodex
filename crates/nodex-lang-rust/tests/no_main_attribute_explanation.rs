use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn explains_no_main_on_the_enclosing_file_module_not_other_items() {
    let source = "#![no_main]\n#![no_std]\nmod inner { fn f() {} } fn entry() {}";
    let file = lower_source("src/main.rs", source);
    let root = file.symbols.iter().find(|s| s.kind == SymbolKind::Module && s.parent.is_none()).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "No generated main entry point").collect();
    assert_eq!(items.len(), 1, "{:?}", root.rust_contract);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "#![no_main]");
    assert!(items[0].text.contains("crate root"));
    assert!(items[0].text.contains("does not itself provide an entry point"));
    assert!(items[0].text.contains("no_std"));
    for name in ["inner", "entry"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "No generated main entry point"));
    }
    let negative = lower_source("src/main.rs", "#![no_std] #![cfg_attr(test, no_main)] fn main() {}");
    let root = negative.symbols.iter().find(|s| s.kind == SymbolKind::Module && s.parent.is_none()).unwrap();
    assert!(!root.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "No generated main entry point"));
}
