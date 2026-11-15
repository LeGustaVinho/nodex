use nodex_lang_rust::lower_source;

#[test]
fn outer_comment_and_doc_attribute_explain_same_documentation() {
    let source = "/// text\nfn from_comment() {}\n#[doc = \" text\"]\nfn from_attr() {}\n//// ordinary\nfn ordinary() {}\nmod inner { //! inner\nfn inside() {} }";
    let file = lower_source("src/lib.rs", source);
    for (name, written) in [("from_comment", "/// text"), ("from_attr", "#[doc = \" text\"]")] {
        let symbol = file.symbols.iter().find(|s| s.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Outer documentation equivalence").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        assert_eq!(source[items[0].span.start_byte as usize..items[0].span.end_byte as usize].trim_end(), written);
        for part in ["/// text", "#[doc = \" text\"]", "following item", "//!", "runtime"] { assert!(items[0].text.contains(part), "{}", items[0].text); }
    }
    for name in ["ordinary", "inside"] {
        let symbol = file.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Outer documentation equivalence"));
    }
}
