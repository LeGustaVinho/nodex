use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn none_pattern_is_explained_without_assuming_option_resolution() {
    let source = "fn inspect(x: Option<i32>) { match x { None => 0, Some(v) => v }; if let None = x { work(); } } fn expression() { let x = None; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "inspect").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Understand a `None` pattern")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "None");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("If it resolves") && text.contains("new binding") && text.contains("name resolution")));
    }
    let expression = ir.flows.iter().find(|flow| flow.name == "expression").unwrap();
    assert!(!expression.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Understand a `None` pattern")));
}
