use nodex_lang_rust::lower_source;

#[test]
fn simple_feature_cfg_is_explained_without_assuming_current_build_configuration() {
    let source = r#"#[cfg(feature = "foo")] fn enabled() {} #[cfg(feature="bar",)] struct S; #[cfg(any(feature="foo", test))] fn compound() {} #[cfg_attr(feature="foo", inline)] fn conditional_attribute() {} #[cfg(feature = "outer")] impl S { fn method(&self) {} }"#;
    let ir = lower_source("src/lib.rs", source);
    for (name, predicate) in [("enabled", "feature = \"foo\""), ("S", "feature=\"bar\""), ("method", "feature = \"outer\"")] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Feature-gated compilation").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], predicate);
        for phrase in ["If it is true", "If false", "other conditions can still exclude", "build-time selection", "not a runtime if", "Cargo", "does not establish"] {
            assert!(items[0].text.contains(phrase), "{name}: {}", items[0].text);
        }
        assert!(symbol.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Build condition"));
    }
    for name in ["compound", "conditional_attribute"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Feature-gated compilation"), "{name}");
    }
}
