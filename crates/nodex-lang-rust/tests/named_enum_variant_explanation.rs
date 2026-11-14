use nodex_lang_rust::lower_source;

#[test]
fn explains_named_fields_on_enum_and_variant_symbols() {
    let ir = lower_source("src/lib.rs", "enum Shape { Circle { radius: f32 }, Point, Other(u32), Empty {} }");
    let shape = ir.symbols.iter().find(|s| s.name == "Shape").unwrap();
    let circle = ir.symbols.iter().find(|s| s.name == "Circle").unwrap();
    for symbol in [shape, circle] {
        let item = symbol.rust_contract.as_ref().unwrap().iter()
            .find(|item| item.group == "Named-field enum variant" && item.text.contains("Circle"))
            .expect("named variant should have a visible explanation");
        assert!(item.text.contains("radius: f32"));
        assert!(item.text.contains("one variant at a time"));
        assert!(item.text.contains("does not create a value"));
    }
    for name in ["Point", "Other"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Named-field enum variant")));
    }
    let empty = ir.symbols.iter().find(|s| s.name == "Empty").unwrap();
    assert!(empty.rust_contract.as_ref().unwrap().iter().any(|item| item.text.contains("braces contain no fields")));
}
