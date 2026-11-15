use nodex_lang_rust::lower_source;

#[test]
fn non_exhaustive_struct_explains_defining_and_external_crate_rules() {
    let source = r#"
        #[non_exhaustive] pub struct Config { pub field: i32 }
        #[non_exhaustive] #[derive(Clone)] pub struct Tuple(pub i32);
        #[non_exhaustive] pub struct Unit;
        #[cfg_attr(test, non_exhaustive)] struct Conditional;
        #[non_exhaustive(extra)] struct Invalid;
        #[non_exhaustive] enum Other { A }
        struct Plain;
    "#;
    let ir = lower_source("src/lib.rs", source);
    for name in ["Config", "Tuple", "Unit"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Non-exhaustive struct").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[non_exhaustive]");
        for phrase in ["future fields", "Inside the defining crate", "Outside the defining crate", "struct update syntax", "tuple and unit struct constructors", "braced struct pattern containing `..`", "does not itself provide a constructor"] {
            assert!(item.text.contains(phrase), "{name}: {}", item.text);
        }
    }
    for name in ["Conditional", "Invalid", "Other", "Plain"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Non-exhaustive struct"), "{name}");
    }
}
