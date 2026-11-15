use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn matches_macro_is_explained_conditionally_in_flow_and_constants() {
    let source = "const C: bool = matches!(Some(2), Some(_)); fn f(x: Option<i32>) { let yes = matches!(x, Some(_)); let other = core::matches!(x, Some(v) if v > 0); } fn g() { let x = custom!(1); }";
    let ir = lower_source("src/lib.rs", source);
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "C").unwrap();
    let item = constant.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Conditional pattern-test macro").expect("constant macro explanation");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "!");
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Conditional pattern-test macro")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "!");
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        assert!(text.contains("If it resolves to Rust's standard matches!"));
        assert!(text.contains("evaluated once"));
        assert!(text.contains("guard is checked only after a match"));
        assert!(text.contains("custom macro with the same name can behave differently"));
    }
    let other = ir.flows.iter().find(|flow| flow.name == "g").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Conditional pattern-test macro")));
}
