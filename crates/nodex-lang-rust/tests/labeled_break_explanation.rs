use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn named_break_explains_exiting_outer_loop_and_excludes_other_forms() {
    let source = "fn f() { 'outer: loop { loop { break 'outer; } } 'inner: loop { break; } 'block: { break 'block; } }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Exit a named enclosing destination")).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    let outer = items.iter().find(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "'outer").unwrap();
    assert!(matches!(&outer.content, ReadingContent::Detail { text, .. } if text.contains("after that loop") && text.contains("apostrophe") && text.contains("labeled blocks")));
}
