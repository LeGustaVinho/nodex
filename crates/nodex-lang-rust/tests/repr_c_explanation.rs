use nodex_lang_rust::lower_source;

#[test]
fn explains_c_representation_according_to_struct_union_and_enum_kind() {
    let source = "#[repr(C)] struct S { a: u8, b: u32 } #[repr(C)] union U { a: u8, b: u32 } #[repr(C)] enum E { A, B } #[repr(C)] enum Data { A(i32), B } #[repr(transparent)] struct T(u32); #[repr(C, packed)] struct P { x: u32 } #[cfg_attr(feature = \"x\", repr(C))] struct Cond;";
    let file = lower_source("src/lib.rs", source);
    for (name, expected) in [("S", "fields in declaration order"), ("U", "fields overlap"), ("E", "C enum representation"), ("Data", "tag and payload layout")] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "C representation").expect("repr contract");
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[repr(C)]");
        assert!(item.text.contains(expected), "{}", item.text);
        assert!(item.text.contains("does not make every field FFI-safe"));
        assert!(item.text.contains("compilation target"));
    }
    for name in ["T", "P", "Cond"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "C representation"));
    }
}
