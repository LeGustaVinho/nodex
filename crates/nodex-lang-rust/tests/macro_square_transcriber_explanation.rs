use nodex_lang_rust::lower_source;

#[test]
fn explains_square_delimiters_on_the_macro_symbol_without_claiming_an_array() {
    let source = "macro_rules! m { ($x:expr) => [ $x ]; } macro_rules! array { ($x:expr) => { [$x] }; } macro_rules! types { ($x:ty) => [$x]; }";
    let file = lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|symbol| symbol.name == "m!").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Macro rule with square-delimited expansion").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "($x:expr) => [ $x ]");
    assert!(item.text.contains("not themselves emitted as array brackets"));
    assert!(item.text.contains("matcher does not evaluate"));
    assert!(item.text.contains("$x"));
    for name in ["array!", "types!"] {
        let other = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Macro rule with square-delimited expansion"));
    }
}
