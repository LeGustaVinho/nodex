use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn env_macro_lookup_is_conditional_and_visible_without_confusing_other_macros() {
    let source = r##"fn f() { let a = env!("NAME"); let b = custom::env!("OTHER", "missing"); let c = option_env!("NAME"); let d = env!["NAME"]; }"##;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Compile-time environment lookup")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in &sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "(");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("If ") && text.contains("during compilation") && text.contains("compile-time error") && text.contains("not a runtime") && text.contains("does not prove")));
    }
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("custom::env"))));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Invoke a macro with parentheses")));
}
