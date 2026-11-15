use nodex_lang_rust::lower_source;

#[test]
fn stdcall_function_explains_target_abi_and_linking_limits() {
    let source = "extern \"stdcall\" fn callback() {} pub unsafe extern \"stdcall\" fn unsafe_callback() {} extern \"system\" fn system() {} extern \"stdcall-unwind\" fn unwind() {} fn normal() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["callback", "unsafe_callback"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Stdcall calling convention").unwrap_or_else(|| panic!("{:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "extern \"stdcall\"");
        assert!(item.text.contains("only for 32-bit x86") && item.text.contains("does not prove"));
        assert!(item.text.contains("does not by itself load a library") && item.text.contains("FFI safety"));
    }
    for name in ["system", "unwind", "normal"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Stdcall calling convention"));
    }
}
