use nodex_lang_rust::lower_source;

#[test]
fn explains_path_and_named_string_without_assuming_attribute_semantics() {
    let source = "#[attr(path::name, key = \"value\")] struct S; #[attr(key = \"value\")] struct Other; #[attr(\"path::name, key = value\")] struct StringOnly;";
    let file = lower_source("src/lib.rs", source);
    let s = file.symbols.iter().find(|symbol| symbol.name == "S").unwrap();
    let item = s.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Attribute with path and named input").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[attr(path::name, key = \"value\")]");
    assert!(item.text.contains("path::name") && item.text.contains("key = \"value\"") && item.text.contains("not a runtime assignment") && item.text.contains("depend on the resolved attribute"));
    for name in ["Other", "StringOnly"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Attribute with path and named input"));
    }
}
