use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_assignment_of_unsafe_c_function_pointer() {
    let source = "fn f() { let f: unsafe extern \"C\" fn(i32) = foreign; let safe: extern \"C\" fn(i32) = foreign; let rust: unsafe fn(i32) = foreign; let unwind: unsafe extern \"C-unwind\" fn(i32) = foreign; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Bind an unsafe C function pointer")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "unsafe extern \"C\" fn(i32)");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("binds f to the value of foreign") && text.contains("initializer does not call") && text.contains("calling convention") && text.contains("unsafe context") && text.contains("does not resolve")));
}
