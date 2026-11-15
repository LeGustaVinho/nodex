use nodex_lang_rust::lower_source;

#[test]
fn global_asm_is_explained_in_its_module_without_assuming_resolution() {
    let source = "core::arch::global_asm!(\".global entry\", \"entry:\", \"ret\"); other::global_asm!(\"custom\"); core::arch::asm!(\"nop\"); mod inner { core::arch::global_asm!(\"nested\"); } fn ordinary() {}";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.parent.is_none()).unwrap();
    let inner = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.name == "inner").unwrap();
    let root_facts: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Global assembly macro").collect();
    let inner_facts: Vec<_> = inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Global assembly macro").collect();
    assert_eq!(root_facts.len(), 2, "{:?}", root.rust_contract);
    assert_eq!(inner_facts.len(), 1, "{:?}", inner.rust_contract);
    assert!(root_facts.iter().all(|item| item.text.contains("If `") && item.text.contains("outside a function") && item.text.contains("not executed like a runtime function call") && item.text.contains("target, assembler") && item.text.contains("do not prove")));
    assert!(root_facts.iter().any(|item| item.text.contains("core::arch::global_asm") && item.text.contains("const") && item.text.contains("sym")));
    assert!(inner_facts[0].text.contains("nested"));
    assert!(root_facts.iter().all(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize] == "global_asm"));
    let ordinary = ir.symbols.iter().find(|s| s.name == "ordinary").unwrap();
    assert!(!ordinary.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Global assembly macro"));
}
