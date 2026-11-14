use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_inline_const_blocks_on_flow_without_confusing_plain_blocks() {
    let source = "fn f<T>() -> usize { let a = const { 1 + 1 }; let b = const { core::mem::size_of::<T>() }; let c = { 3 }; a + b + c }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Evaluate an inline constant")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "const");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("compile-time") && text.contains("generic parameters") && text.contains("runtime variables") && text.contains("need not necessarily be evaluated")));
    }
}
