use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_option_env_conditionally_without_matching_other_macros() {
    let source = "fn f() { let x = option_env!(\"NAME\"); let y = env!(\"NAME\"); let z = custom::option_env!(\"NAME\"); }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Optional compile-time environment variable"
    )).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for site in items {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "!");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("If it resolves") && text.contains("while compiling") && text.contains("Some(value)") && text.contains("None") && text.contains("not a lookup when the program runs")));
    }
}
