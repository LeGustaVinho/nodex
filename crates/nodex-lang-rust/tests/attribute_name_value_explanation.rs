use nodex_lang_rust::lower_source;
#[test]
fn direct_name_value_attributes_preserve_the_written_value_and_attach_to_the_next_item() {
    let source = r##"#[attribute = "value"] // note
#[doc = "two  spaces"] pub fn target() {} fn ordinary() {} #[named(foo = "not a direct value")] fn nested() {} #[cfg_attr(unix, other = "conditional")] fn conditional() {}"##;
    let ir = lower_source("src/lib.rs", source);
    let target = ir.symbols.iter().find(|symbol| symbol.name == "target").unwrap();
    let items: Vec<_> = target.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Name-value attribute").collect();
    assert_eq!(items.len(), 2, "{:?}", target.rust_contract);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], r#"#[attribute = "value"]"#);
    assert!(items[0].text.contains("name is attribute") && items[0].text.contains(r#"written value is "value""#) && items[0].text.contains("not an assignment") && items[0].text.contains("does not establish"));
    assert!(items[1].text.contains(r#""two  spaces""#));
    for name in ["ordinary", "nested", "conditional"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Name-value attribute"));
    }
}
