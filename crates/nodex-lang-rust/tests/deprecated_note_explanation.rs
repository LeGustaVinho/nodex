use nodex_lang_rust::lower_source;

#[test]
fn deprecated_note_is_visible_with_authored_message_and_respects_scope() {
    let source = "#[deprecated(note = \"Use new_api\")] fn old() {} #[deprecated(since = \"1.0\", note = r#\"Use replacement\"#)] struct Old; #[deprecated] fn no_note() {} fn normal() {}";
    let file = lower_source("src/lib.rs", source);
    for (name, message) in [("old", "\"Use new_api\""), ("Old", "r#\"Use replacement\"#")] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Deprecation notice").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!((&source[item.span.start_byte as usize..item.span.end_byte as usize]).starts_with("#[deprecated("));
        assert!(item.text.contains(message));
        assert!(item.text.contains("warning rather than removing"));
        assert!(item.text.contains("lint settings"));
        assert!(item.text.contains("does not itself call the replacement"));
    }
    for name in ["no_note", "normal"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Deprecation notice"));
    }
}
