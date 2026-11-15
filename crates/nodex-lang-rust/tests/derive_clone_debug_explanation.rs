use nodex_lang_rust::lower_source;

#[test]
fn explains_clone_debug_derive_on_the_type_without_conflating_other_attributes() {
    let source = "#[derive(Clone, Debug)]\n#[allow(dead_code)]\nstruct S { value: i32 } #[derive(Debug)] struct D; #[cfg_attr(feature = \"x\", derive(Clone, Debug))] struct C;";
    let file = lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|symbol| symbol.name == "S").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Derive Clone and Debug").expect("derive contract");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[derive(Clone, Debug)]");
    assert!(item.text.contains("apply the listed derive macros"));
    assert!(item.text.contains("If Clone and Debug resolve"));
    assert!(item.text.contains("do not automatically call clone"));
    for name in ["D", "C"] {
        let other = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Derive Clone and Debug"));
    }
}
