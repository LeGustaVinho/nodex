use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_empty_tuple_binding_without_confusing_other_patterns() {
    let source = "fn f() { let () = (); let (x,) = (1,); let _ = (); let () = work(); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Unit binding pattern")).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "()");
        assert!(matches!(&item.content, ReadingContent::Detail { text, .. } if text.contains("only value of the unit type") && text.contains("introduces no names") && text.contains("evaluated first") && text.contains("wildcard")));
    }
}
