use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_mutable_binding_in_destructuring_without_confusing_ref_mut() {
    let text = "fn f(pair: (i32, i32), value: Option<i32>) { let (mut x, y) = pair; x = y; match value { Some(ref mut v) => {}, None => {} } }";
    let ir = lower_source("src/lib.rs", text);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Mutable pattern binding")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    assert_eq!(&text[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "mut");
    assert!(matches!(&facts[0].content, ReadingContent::Detail { text, .. }
        if text.contains("binding named x") && text.contains("not a mutable borrow") && text.contains("Other names")));
}
