use nodex_lang_rust::lower_source;

#[test]
fn explains_expr_2021_in_matchers_and_repetitions_but_not_transcribers() {
    let source = "macro_rules! m { ($x:expr_2021) => { $x }; ($( $v:expr_2021 ),*) => { stringify!($v:expr_2021) }; ($n:expr) => { $n }; } macro_rules! plain { ($x:expr) => { stringify!($fake:expr_2021) }; }";
    let ir = lower_source("src/lib.rs", source);
    let mac = ir.symbols.iter().find(|symbol| symbol.name == "m!").unwrap();
    let facts: Vec<_> = mac.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro expr_2021 fragment").collect();
    assert_eq!(facts.len(), 2, "{:?}", mac.rust_contract);
    assert_eq!(&source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "$x:expr_2021");
    assert_eq!(&source[facts[1].span.start_byte as usize..facts[1].span.end_byte as usize], "$v:expr_2021");
    for fact in &facts {
        assert!(fact.text.contains("top-level `_`") && fact.text.contains("`const { ... }`") && fact.text.contains("inside a larger expression"));
        assert!(fact.text.contains("edition of the macro definition") && fact.text.contains("does not evaluate"));
    }
    assert!(facts[1].text.contains("inside a matcher repetition"));
    let plain = ir.symbols.iter().find(|symbol| symbol.name == "plain!").unwrap();
    assert!(!plain.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Macro expr_2021 fragment"));
}
