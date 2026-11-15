use nodex_lang_rust::lower_source;

#[test]
fn comma_plus_expression_repetition_is_explained_only_in_matchers() {
    let source = "macro_rules! list { ($($x:expr),+) => { () }; } macro_rules! nested { ($( $( $y:expr ),+ );+) => { () }; } macro_rules! negatives { ($($a:expr),*) => { () }; ($($b:expr);+) => { () }; ($n:expr) => { $($n),+ }; }";
    let ir = lower_source("src/lib.rs", source);
    for (name, metavariable) in [("list!", "$x"), ("nested!", "$y")] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "One-or-more expression repetition").collect();
        assert_eq!(facts.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = facts[0];
        let written = &source[item.span.start_byte as usize..item.span.end_byte as usize];
        assert!(written.contains(&format!("{metavariable}:expr")) && written.ends_with(",+"), "{written}");
        assert!(item.text.contains("one or more"));
        assert!(item.text.contains("empty list does not satisfy"));
        assert!(item.text.contains("does not consume a trailing comma"));
        assert!(item.text.contains("does not execute"));
    }
    let negative = ir.symbols.iter().find(|s| s.name == "negatives!").unwrap();
    assert!(!negative.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "One-or-more expression repetition"));
}
