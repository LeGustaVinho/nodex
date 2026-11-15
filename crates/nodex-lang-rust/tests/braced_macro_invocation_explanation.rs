use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_braced_macro_without_treating_other_delimiters_as_braces() {
    let source = "fn f(value: i32) { name! { value } name!(value); name![value]; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Invoke a macro with braces")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let site = sites[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "{");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("name!") && text.contains("tokens supplied") && text.contains("not necessarily a runtime code block")
        && text.contains("depend on the macro's definition")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Macro invocation")));
}
