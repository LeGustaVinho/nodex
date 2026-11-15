use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn closure_typed_tuple_is_one_argument_split_into_two_bindings() {
    let source = "fn f() { let sum = |(x, y): (i32, i32)| x + y; let other = |x: i32, y: i32| x + y; let bare = |(x, y)| x + y; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Typed tuple closure parameter")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "(");
    let ReadingContent::Detail { text, .. } = &item.content else { unreachable!() };
    for phrase in ["one closure parameter, not 2 separate arguments", "element 0 binds `x` with type `i32`", "element 1 binds `y` with type `i32`", "runs on each call", "not a cast", "copy or move"] { assert!(text.contains(phrase), "{text}"); }
}
