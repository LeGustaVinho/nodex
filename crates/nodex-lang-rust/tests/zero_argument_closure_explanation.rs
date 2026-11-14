use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_empty_parameter_bars_and_deferred_expression_body() {
    let source = "fn f() { let c = || 42; let with_arg = |x| x + 1; let later = async || 42; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Closure with no parameters")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let site = sites[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "42");
    let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
    assert!(text.contains("takes no arguments"));
    assert!(text.contains("does not execute"));
    assert!(text.contains("returns 42 when called"));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label.starts_with("Create function c"))));
}
