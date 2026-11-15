use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn explains_four_element_array_trait_impl_in_module_and_member() {
    let source = "trait Trait { fn f(); } impl<T> Trait for [T; 4] { fn f() {} } mod nested { trait Other {} impl<A> Other for [A; 4] {} } trait Small {} impl<T> Small for [T; 3] {}";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.id == "ns:crate::lib").or_else(|| ir.symbols.iter().find(|symbol| symbol.kind == SymbolKind::Module)).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Trait implementation for a four-element array").collect();
    assert_eq!(items.len(), 1, "{:?}", root.rust_contract);
    let item = items[0];
    assert!(item.text.contains("[T; 4]") && item.text.contains("different length") && item.text.contains("orphan rules"));
    let member = ir.symbols.iter().find(|symbol| symbol.name == "f" && symbol.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Trait implementation for a four-element array"))).unwrap();
    assert!(member.rust_contract.is_some());
    let nested = ir.symbols.iter().find(|symbol| symbol.name == "nested" && symbol.kind == SymbolKind::Module).unwrap();
    let nested_items: Vec<_> = nested.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Trait implementation for a four-element array").collect();
    assert_eq!(nested_items.len(), 1);
    assert!(nested_items[0].text.contains("[A; 4]") && nested_items[0].text.contains("Other"));
}
