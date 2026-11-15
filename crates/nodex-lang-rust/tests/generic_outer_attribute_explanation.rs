use nodex_lang_rust::lower_source;

#[test]
fn outer_attribute_is_explained_on_its_item_without_guessing_its_effect() {
    let source = "#![allow(dead_code)]\n#[attribute]\n/// Details\n#[other(value)]\nfn marked() {}\nfn plain() {}\nstruct S { #[attribute] field: i32, other: i32 }\nenum E { #[attribute] V, W }";
    let ir = lower_source("src/lib.rs", source);
    let marked = ir.symbols.iter().find(|symbol| symbol.name == "marked").unwrap();
    let attrs: Vec<_> = marked.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Outer attribute").collect();
    assert_eq!(attrs.len(), 2, "{:?}", marked.rust_contract);
    assert_eq!(&source[attrs[0].span.start_byte as usize..attrs[0].span.end_byte as usize], "#[attribute]");
    assert_eq!(&source[attrs[1].span.start_byte as usize..attrs[1].span.end_byte as usize], "#[other(value)]");
    assert!(attrs[0].text.contains("declaration that follows") && attrs[0].text.contains("alone does not establish") && attrs[0].text.contains("#![...]"));
    for name in ["field", "V"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Outer attribute"), "missing {name}");
    }
    for name in ["plain", "other", "W"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Outer attribute"), "unexpected {name}");
    }
}
