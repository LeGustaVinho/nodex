use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn open_ended_index_explains_array_slice_conditionally_and_preserves_range_detail() {
    let source = "fn f(array: [i32; 4]) { let suffix = &array[2..]; let prefix = &array[..2]; let inclusive = &array[2..=3]; let element = array[2]; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Index from a starting position to the end")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    let site = facts[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "[");
    let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
    assert!(text.contains("If the receiver is a standard array or slice"));
    assert!(text.contains("position 2 through the end"));
    assert!(text.contains("equal to the length selects an empty slice"));
    assert!(text.contains("past the length panics"));
    assert!(text.contains("Indexing can be overloaded"));
    assert!(text.contains("often borrowed with & or &mut"));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { text, .. } if text.contains("Start at 2 with no specified end"))), "{:?}", flow.reading);
}
