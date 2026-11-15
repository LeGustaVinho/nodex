use nodex_lang_rust::lower_source;
#[test]
fn unsafe_naked_explains_missing_prologue_and_assembly_obligations() {
    let source = "#[unsafe(naked)] // note\n#[allow(dead_code)] pub unsafe extern \"C\" fn entry() { core::arch::naked_asm!(\"ret\"); } struct S; impl S { #[unsafe ( naked )] unsafe extern \"C\" fn method() { core::arch::naked_asm!(\"ret\"); } } #[unsafe(no_mangle)] extern \"C\" fn plain() {} #[cfg_attr(test, unsafe(naked))] fn conditional() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["entry", "method"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Unsafe naked function").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        assert!(source[items[0].span.start_byte as usize..items[0].span.end_byte as usize].starts_with("#[unsafe"));
        assert!(items[0].text.contains("no usual function prologue or epilogue") && items[0].text.contains("naked_asm!") && items[0].text.contains("calling convention") && items[0].text.contains("does not prove") && items[0].text.contains("cannot use the Rust ABI"));
    }
    for name in ["plain", "conditional"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Unsafe naked function"));
    }
}
