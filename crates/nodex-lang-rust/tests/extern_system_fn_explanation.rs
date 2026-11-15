use nodex_lang_rust::lower_source;

#[test]
fn system_abi_definition_explains_target_dependence_without_assuming_a_library() {
    let source = "extern \"system\" fn callback() {} pub unsafe extern \"system\" fn unsafe_callback() {} extern \"C\" fn c_callback() {} extern \"system-unwind\" fn unwind_callback() {} fn ordinary() {}";
    let file = lower_source("src/lib.rs", source);
    for name in ["callback", "unsafe_callback"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "System calling convention").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "extern \"system\"");
        assert!(item.text.contains("depends on the compilation target"));
        assert!(item.text.contains("32-bit x86 Windows"));
        assert!(item.text.contains("not by itself a declaration that loads"));
        assert!(item.text.contains("does not guarantee"));
    }
    for name in ["c_callback", "unwind_callback", "ordinary"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "System calling convention"));
    }
}
