use nodex_lang_rust::lower_source;

#[test]
fn deny_unused_variables_explains_effective_lint_level_without_assuming_a_diagnostic() {
    let source = r#"
        #[deny(unused_variables)] fn checked() { let unused = 1; }
        #[deny(unused_mut, unused_variables, reason = "why")] fn multiple() {}
        struct S; #[deny(unused_variables)] impl S { fn method(&self) {} }
        #[warn(unused_variables)] fn warning() {}
        #[deny(unused_mut)] fn other() {}
        #[deny(clippy::unused_variables)] fn tool_lint() {}
        #[cfg_attr(test, deny(unused_variables))] fn conditional() {}
        #[deny(unused_mut, reason = "unused_variables")] fn reason_only() {}
    "#;
    let ir = lower_source("src/lib.rs", source);
    for name in ["checked", "multiple", "method"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Deny unused-variable lint").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = items[0];
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "unused_variables");
        for phrase in ["variable bindings", "effective deny level", "error", "Inner lint attributes", "lint caps", "does not prove", "not a runtime instruction"] {
            assert!(item.text.contains(phrase), "{name}: {}", item.text);
        }
    }
    for name in ["warning", "other", "tool_lint", "conditional", "reason_only"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Deny unused-variable lint"), "{name}");
    }
}
