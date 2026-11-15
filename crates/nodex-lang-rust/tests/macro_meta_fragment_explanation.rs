use nodex_lang_rust::lower_source;

#[test]
fn meta_fragment_explains_matcher_and_nested_repetition_but_not_transcriber() {
    let source = "macro_rules! attrs { ($(#[$x:meta])*) => { $(#[$x])* struct S; }; ($y:meta) => { #[$y] struct T; }; } macro_rules! other { ($x:expr) => { $x }; ($i:ident) => { $i:meta }; }";
    let ir = lower_source("src/lib.rs", source);
    let attrs = ir.symbols.iter().find(|s| s.name == "attrs!").unwrap();
    let facts: Vec<_> = attrs.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro meta fragment").collect();
    assert_eq!(facts.len(), 2, "{:?}", attrs.rust_contract);
    for (written, repeated) in [("$x:meta", true), ("$y:meta", false)] {
        let fact = facts.iter().find(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize] == written).unwrap();
        assert!(fact.text.contains("attribute's contents as syntax"));
        assert!(fact.text.contains("surrounding #[ and ] are written separately"));
        assert!(fact.text.contains("does not itself apply an attribute"));
        assert_eq!(fact.text.contains("inside a repetition"), repeated);
    }
    let other = ir.symbols.iter().find(|s| s.name == "other!").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Macro meta fragment"));
}
