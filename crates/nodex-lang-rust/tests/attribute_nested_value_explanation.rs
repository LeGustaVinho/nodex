use nodex_lang_rust::lower_source;

#[test]
fn named_string_attribute_input_is_preserved_without_guessing_resolution() {
    let source = "#[attribute(key = \"value\")] struct S; #[tool::attribute(key = \"a b\", flag = \"x\",)] fn f() {} #[attribute(key)] struct Other; #[attribute = \"value\"] struct Eq;";
    let file = lower_source("src/lib.rs", source);
    for (name, input) in [("S", "(key = \"value\")"), ("f", "(key = \"a b\", flag = \"x\",)")] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Attribute with named input").expect("attribute contract");
        assert!(item.text.contains(input), "{}", item.text);
        assert!(item.text.contains("whether this is accepted depend on the resolved attribute"));
        assert!(item.text.contains("does not establish"));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("#["));
    }
    for name in ["Other", "Eq"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Attribute with named input"));
    }
}
