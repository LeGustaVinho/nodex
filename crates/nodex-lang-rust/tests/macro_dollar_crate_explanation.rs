use nodex_lang_rust::lower_source;

#[test]
fn defining_crate_paths_are_explained_only_in_transcriber_tokens() {
    let source = "macro_rules! m { ($x:tt) => { $crate::helper(); nested!($crate::other); \"$crate::fake\" }; ($crate:ident) => { $crate }; }";
    let ir = lower_source("src/lib.rs", source);
    let m = ir.symbols.iter().find(|symbol| symbol.name == "m!").expect("macro symbol");
    let items: Vec<_> = m.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro defining-crate path").collect();
    assert_eq!(items.len(), 2, "{:?}", m.rust_contract);
    for item in &items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "$crate");
        assert!(item.text.contains("crate that defines this macro"));
        assert!(item.text.contains("does not bypass visibility"));
        assert!(item.text.contains("does not call or resolve"));
    }
    assert!(items[0].text.contains("$crate::helper"));
    assert!(items[1].text.contains("$crate::other"));
}
