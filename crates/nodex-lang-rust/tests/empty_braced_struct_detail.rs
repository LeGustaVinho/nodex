use nodex_lang_rust::lower_source;

#[test]
fn empty_braced_struct_is_distinguished_from_unit_and_nonempty_structs() {
    let source = "struct Empty {} struct Unit; struct Tuple(); struct Full { x: i32 } struct Comment { /* none */ }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["Empty", "Comment"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Empty braced struct").unwrap();
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with('{'));
        assert!(item.text.contains("no fields") && item.text.contains("constructed with empty braces") && item.text.contains("bare name") && item.text.contains("unit type `()`"));
    }
    for name in ["Unit", "Tuple", "Full"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Empty braced struct"));
    }
}
