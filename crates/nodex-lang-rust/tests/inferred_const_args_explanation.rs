use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_inference_and_constant_in_type_turbofish_without_assuming_resolution() {
    let source = "fn f() { Type::<_, 3>::new(); Type::<u8, 3>::new(); other::<_, 4>(); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Inferred and constant generic arguments")).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], ",");
        assert!(matches!(&item.content, ReadingContent::Detail { text, .. } if text.contains("two generic arguments in order") && text.contains("type or an inferred constant") && text.contains("not an ordinary runtime call argument") && text.contains("Name resolution")));
    }
}
