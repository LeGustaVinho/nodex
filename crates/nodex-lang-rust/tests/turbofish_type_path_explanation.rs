use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_explicit_type_arguments_on_the_qualifying_path() {
    let source = "fn f() { Type::<u32>::method(); foo::<u32>(); value.method::<u32>(); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Generic arguments on a type path")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "<u32>");
    assert!(matches!(&details[0].content, ReadingContent::Detail { text, .. } if text.contains("not necessarily") && text.contains("name resolution")));
}
