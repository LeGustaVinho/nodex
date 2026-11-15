use nodex_lang_rust::lower_source;

#[test]
fn unsafe_export_name_preserves_literal_and_explains_linker_and_safety() {
    let source = r###"
        #[unsafe(export_name = "my_symbol")] pub extern "C" fn exported() {}
        #[unsafe(export_name = r#"raw_symbol"#)] static VALUE: u8 = 1;
        #[unsafe(export_name = "escaped\u{5f}symbol")] fn escaped() {}
        #[cfg_attr(test, unsafe(export_name = "conditional"))] fn conditional() {}
        #[unsafe(no_mangle)] fn other() {}
        #[unsafe(export_name = concat!("generated", "_name"))] fn generated() {}
    "###;
    let ir = lower_source("src/lib.rs", source);
    for (name, literal) in [("exported", "\"my_symbol\""), ("VALUE", "r#\"raw_symbol\"#"), ("escaped", "\"escaped\\u{5f}symbol\"")] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Unsafe exported symbol name").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], literal);
        assert!(item.text.contains(literal));
        for phrase in ["linker symbol name", "string-literal rules", "Rust item name is unchanged", "does not collide", "cannot prove", "calling convention"] {
            assert!(item.text.contains(phrase), "{name}: {}", item.text);
        }
    }
    for name in ["conditional", "other", "generated"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Unsafe exported symbol name"), "{name}");
    }
}
