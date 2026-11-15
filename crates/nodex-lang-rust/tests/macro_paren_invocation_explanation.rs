use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn parenthesized_macro_invocation_explains_tokens_and_uncertain_evaluation() {
    let source = "fn f(value: i32) { let x = name!(value); name!(value); let y = name![value]; let z = name!{value}; let q = module::name!(value); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Invoke a macro with parentheses")).collect();
    assert_eq!(sites.len(), 3, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "!");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("not logical negation") && text.contains("tokens") && text.contains("not necessarily evaluated") && text.contains("neither evaluation count nor order")));
    }
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Macro invocation")), "{:?}", flow.reading);
}
