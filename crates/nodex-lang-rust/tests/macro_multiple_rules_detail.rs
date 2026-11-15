use nodex_lang_rust::lower_source;
#[test]
fn empty_and_literal_macro_rules_explain_selection_and_empty_output() {
    let source = "macro_rules! m { () => {}; (x) => {} } macro_rules! other { () => {}; ($x:expr) => {} } fn f() { m!(); m!(x); }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|s| s.name == "m!").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Macro with empty and literal-token rules").unwrap();
    assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("macro_rules! m"));
    assert!(item.text.contains("tries them in order") && item.text.contains("literal token `x`") && item.text.contains("does not bind") && item.text.contains("emits no tokens") && item.text.contains("not an empty Rust block") && item.text.contains("later rules are not tried") && item.text.contains("syntactic context"));
    assert!(symbol.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Declarative macro definition"));
    let other = ir.symbols.iter().find(|s| s.name == "other!").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Macro with empty and literal-token rules"));
}
