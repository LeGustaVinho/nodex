use nodex_lang_rust::lower_source;

#[test]
fn track_caller_explains_location_propagation_and_indirect_calls() {
    let source = r#"
        #[track_caller] fn first() {}
        #[track_caller] #[inline] fn second() { first(); }
        trait T { #[track_caller] fn tracked(&self); }
        #[cfg_attr(test, track_caller)] fn conditional() {}
        #[track_caller(extra)] fn invalid() {}
        fn plain() {}
    "#;
    let ir = lower_source("src/lib.rs", source);
    for name in ["first", "second", "tracked"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Tracked caller location").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[track_caller]");
        for phrase in ["Location::caller", "nearest untracked function", "not a guaranteed stack trace", "function pointer or virtual call", "definition instead", "trait method", "Rust ABI"] {
            assert!(item.text.contains(phrase), "{name}: {}", item.text);
        }
    }
    for name in ["conditional", "invalid", "plain"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Tracked caller location"), "{name}");
    }
}
