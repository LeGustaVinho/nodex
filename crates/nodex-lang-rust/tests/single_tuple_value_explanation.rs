use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_one_tuple_value_but_not_grouping_or_assignment_target() {
    let source = "fn f(v: i32) { let tuple = (v,); let grouped = (v); let (mut x,) = (0,); (x,) = (1,); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Create a one-element tuple"
    )).collect();
    assert_eq!(details.len(), 3, "{:?}", flow.reading);
    assert!(details.iter().all(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == ","));
    assert!(details.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("(v,) is a tuple") && text.contains("only groups") && text.contains("`.0`"))));
}
