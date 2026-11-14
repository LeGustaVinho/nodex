#[test]
fn inherent_implementation_is_explained_on_its_type() {
    let source = "struct Player; impl Player { fn new() -> Self { Player } }";
    let file = nodex_lang_rust::lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|s| s.name == "Player").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter()
        .find(|item| item.group == "Inherent implementation").unwrap();
    assert!(item.text.contains("adds methods"));
    assert!(item.text.contains("does not create a value"));
    assert_eq!(source[item.span.start_byte as usize..item.span.end_byte as usize].trim_end(), "impl Player");
}
