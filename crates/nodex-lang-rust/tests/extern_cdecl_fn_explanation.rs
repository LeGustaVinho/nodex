use nodex_lang_rust::lower_source;

#[test]
fn explains_cdecl_definition_without_guessing_target_or_library() {
    let source = "extern \"cdecl\" fn f() {} pub unsafe extern \"cdecl\" fn g() {} extern \"C\" fn c() {} extern \"cdecl-unwind\" fn unwind() {} fn plain() {}";
    let file = lower_source("src/lib.rs", source);
    for name in ["f", "g"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Cdecl calling convention").expect("cdecl contract");
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "extern \"cdecl\"");
        assert!(item.text.contains("32-bit x86 targets"));
        assert!(item.text.contains("depends on the compilation target"));
        assert!(item.text.contains("not by itself a native library import"));
        assert!(item.text.contains("does not guarantee FFI safety"));
    }
    for name in ["c", "unwind", "plain"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Cdecl calling convention"));
    }
}
