use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_method_receiver_lookup_without_claiming_resolution() {
    let source = "fn f(object: Thing, a: i32) { object.method(a); object.method::<u8>(a); plain(a); let _ = object.field; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Look up and call a method")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for fact in facts {
        assert_eq!(&source[fact.span.start_byte as usize..fact.span.end_byte as usize], ".");
        assert!(matches!(&fact.content, ReadingContent::Detail { text, .. } if text.contains("receiver") && text.contains("automatically dereference") && text.contains("shared or mutable borrow") && text.contains("does not prove")));
    }
}
