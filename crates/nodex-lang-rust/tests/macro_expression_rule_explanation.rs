use nodex_lang_rust::lower_source;

#[test]
fn macro_expression_capture_and_single_substitution_are_explained_on_symbol() {
    let source = "macro_rules! identity { ($x:expr) => { $x }; ($t:ty) => { $t }; } macro_rules! double { ($x:expr) => { $x + $x }; } fn f() { let x = identity!(1 + 2); }";
    let ir = lower_source("src/lib.rs", source);
    let identity = ir.symbols.iter().find(|s| s.name == "identity!").unwrap();
    let facts: Vec<_> = identity.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro expression capture rule").collect();
    assert_eq!(facts.len(), 1, "{:?}", identity.rust_contract);
    let fact = facts[0];
    assert_eq!(&source[fact.span.start_byte as usize..fact.span.end_byte as usize], "($x:expr) => { $x }");
    assert!(fact.text.contains("$x:expr captures"));
    assert!(fact.text.contains("macro metavariable"));
    assert!(fact.text.contains("substitutes that captured expression syntax"));
    assert!(fact.text.contains("does not execute the expression"));
    assert!(fact.text.contains("do not by themselves mean an extra Rust block"));
    let double = ir.symbols.iter().find(|s| s.name == "double!").unwrap();
    assert!(!double.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Macro expression capture rule"));
}
