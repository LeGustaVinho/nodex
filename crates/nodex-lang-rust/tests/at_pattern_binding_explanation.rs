use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn at_pattern_explains_the_whole_binding_and_coexists_with_pattern_reading() {
    let source = "fn inspect(input: Option<i32>) { match input { x @ Some(_) => use_value(x), _ => fallback() }; let y @ Some(_) = input else { return; }; use_value(y); } fn plain(input: Option<i32>) { match input { Some(_) => {}, _ => {} } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "inspect").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Bind the value while checking a pattern")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "@");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("entire value") && text.contains("move or copy") && text.contains("resolved in context")));
    }
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Pattern { parts, .. } if parts.iter().any(|part| part.binding.as_deref() == Some("x")))));
    let plain = ir.flows.iter().find(|flow| flow.name == "plain").unwrap();
    assert!(!plain.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Bind the value while checking a pattern")));
}
