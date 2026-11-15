use nodex_lang_rust::lower_source;

#[test]
fn expected_panic_message_is_preserved_and_explained_only_for_the_explicit_form() {
    let source = r###"
        #[test] #[should_panic(expected = "boom")] fn ordinary() { panic!("boom more"); }
        #[test] #[should_panic(expected = "say \"boom\"",)] fn escaped() { panic!(); }
        #[test] #[should_panic(expected = r#"raw "text""#)] fn raw() { panic!(); }
        #[should_panic(expected = "standalone")] fn not_a_test() {}
        #[test] #[should_panic] fn any_message() { panic!(); }
        #[cfg_attr(test, should_panic(expected = "conditional"))] fn conditional() {}
        #[should_panic(unexpected = "wrong")] fn wrong_key() {}
    "###;
    let ir = lower_source("src/lib.rs", source);
    for (name, written) in [("ordinary", "\"boom\""), ("escaped", "\"say \\\"boom\\\"\""), ("raw", "r#\"raw \"text\"\"#"), ("not_a_test", "\"standalone\"")] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Expected panic message").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = items[0];
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], written);
        assert!(item.text.contains(written), "{name}: {}", item.text);
        for phrase in ["run as a test", "to panic", "contain the specified string", "need not be the entire message", "escapes interpreted", "returns normally", "does not itself trigger a panic or register"] {
            assert!(item.text.contains(phrase), "{name}: {}", item.text);
        }
    }
    for name in ["any_message", "conditional", "wrong_key"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Expected panic message"), "{name}");
    }
}
