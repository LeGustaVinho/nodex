use nodex_lang_rust::lower_source;
#[test]
fn global_allocator_attribute_explains_selection_and_unverified_restrictions() {
    let source = "use std::alloc::System; #[global_allocator] // note\n#[allow(dead_code)] static GLOBAL: System = System; static PLAIN: System = System; #[global_allocator] fn wrong() {} #[cfg_attr(test, global_allocator)] static CONDITIONAL: System = System; #[other(global_allocator)] static OTHER: System = System;";
    let ir = lower_source("src/lib.rs", source);
    let global = ir.symbols.iter().find(|symbol| symbol.name == "GLOBAL").unwrap();
    let items: Vec<_> = global.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Global allocator selection").collect();
    assert_eq!(items.len(), 1, "{:?}", global.rust_contract);
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[global_allocator]");
    assert!(item.text.contains("static GLOBAL") && item.text.contains("type here is System") && item.text.contains("implements `GlobalAlloc`") && item.text.contains("only once in the crate graph") && item.text.contains("does not prove") && item.text.contains("does not guarantee"));
    for name in ["PLAIN", "wrong", "CONDITIONAL", "OTHER"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Global allocator selection"), "{name}");
    }
}
