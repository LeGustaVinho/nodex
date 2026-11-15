use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn println_positional_placeholder_is_explained_conditionally_and_coexists_with_macro_reading() {
    let source = r#"fn demo(x: i32) { println!("x = {}", x); custom::println!("x = {}", x); print!("x = {}", x); println!("x = {:?}", x); println!("plain"); }"#;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "demo").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Format and print a value")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "{}");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("If ") && text.contains("standard println macro") && text.contains("argument x")
                && text.contains("Display") && text.contains("newline") && text.contains("standard output")
                && text.contains("does not prove") && text.contains("could behave differently")));
    }
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Macro invocation")), "{:?}", flow.reading);
}
