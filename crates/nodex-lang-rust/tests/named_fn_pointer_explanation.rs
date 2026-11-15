use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn named_function_pointer_parameters_are_documentary_and_calls_are_positional() {
    let source = "fn use_it() { let f: fn(x: i32) -> i32 = identity; let g: fn(left: i32, right: bool) -> i32 = compute; let h: fn(i32) -> i32 = identity; let ordinary: i32 = 1; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "use_it").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Named function pointer parameter")).collect();
    assert_eq!(details.len(), 3, "{:?}", flow.reading);
    for (item, name) in details.iter().zip(["x", "left", "right"]) {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], name);
        let ReadingContent::Detail { text, .. } = &item.content else { unreachable!() };
        for phrase in ["documentation name", "does not change the function pointer's type", "declare a local variable", "arguments by position", "full signature", "does not resolve"] {
            assert!(text.contains(phrase), "{text}");
        }
    }
}
