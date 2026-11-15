use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn explains_no_builtins_on_file_module_not_nested_items_or_conditional_attribute() {
    let source = "#![no_builtins]\n#![no_std]\nmod inner { fn f() {} } fn entry() {}";
    let file = lower_source("src/lib.rs", source);
    let root = file.symbols.iter().find(|s| s.kind == SymbolKind::Module && s.parent.is_none()).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Disable selected builtin-call optimizations").collect();
    assert_eq!(items.len(), 1, "{:?}", root.rust_contract);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "#![no_builtins]");
    assert!(items[0].text.contains("crate root"));
    assert!(items[0].text.contains("does not disable all optimizations"));
    assert!(items[0].text.contains("does not establish the actual build configuration"));
    for name in ["inner", "entry"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Disable selected builtin-call optimizations"));
    }
    let negative = lower_source("src/lib.rs", "#![no_std] #![cfg_attr(test, no_builtins)] fn f() {}");
    let root = negative.symbols.iter().find(|s| s.kind == SymbolKind::Module && s.parent.is_none()).unwrap();
    assert!(!root.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Disable selected builtin-call optimizations"));
}
