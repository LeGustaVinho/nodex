use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn macro_token_tree_explains_three_matching_nested_delimiters() {
    let source = "fn f() { m!([x, {y}]); } fn g() { m![z]; m!{q}; } fn no_macro() { let _ = (1, [2]); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Macro token-tree delimiters")).collect();
    assert_eq!(facts.len(), 3, "{:?}", flow.reading);
    for (opening, detail) in [("(", "`(` and `)`"), ("[", "`[` and `]`"), ("{", "`{` and `}`")] {
        let fact = facts.iter().find(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == opening).unwrap();
        let ReadingContent::Detail { text, .. } = &fact.content else { unreachable!() };
        for phrase in [detail, "three paired forms", "nested groups", "not necessarily", "not universally interchangeable"] { assert!(text.contains(phrase), "{text}"); }
    }
    let plain = ir.flows.iter().find(|flow| flow.name == "no_macro").unwrap();
    assert!(!plain.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Macro token-tree delimiters")));
}
