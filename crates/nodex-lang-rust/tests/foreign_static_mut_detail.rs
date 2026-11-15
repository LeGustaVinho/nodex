use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn mutable_foreign_static_is_visible_in_owning_module_contract() {
    let source = "unsafe extern \"C\" { static mut X: i32; static Y: i32; } mod inner { unsafe extern \"C\" { static mut N: u8; } } mod other { unsafe extern \"system\" { static mut S: i32; } } static mut OWN: i32 = 0;";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Mutable foreign static").collect();
    assert_eq!(items.len(), 1, "{items:?}");
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "mut");
    for phrase in ["static mut X: i32", "linked code", "unsafe operations", "data races", "invalid aliasing", "does not provide synchronization", "do not prove"] { assert!(item.text.contains(phrase), "{}", item.text); }
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    assert_eq!(inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Mutable foreign static").count(), 1);
    let other = ir.symbols.iter().find(|symbol| symbol.name == "other").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Mutable foreign static"));
}
