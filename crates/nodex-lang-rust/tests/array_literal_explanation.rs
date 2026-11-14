use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn array_list_explains_length_inference_and_order_without_treating_assignment_as_construction() {
    let source = "fn f() { let a = [1, 2, 3]; let repeated = [0; 3]; let (mut x, mut y) = (0,0); [x,y] = [4,5]; use_value(a,repeated,x,y); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Create a fixed-size array")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    let first = details.iter().find(|site| site.span.start_byte as usize == source.find("[1, 2, 3]").unwrap()).unwrap();
    let ReadingContent::Detail { text, .. } = &first.content else { unreachable!() };
    assert!(text.contains("3 element"));
    assert!(text.contains("left to right"));
    assert!(text.contains("infer that type"));
    assert!(text.contains("length is part of the array's type"));
    let lhs = source.find("[x,y]").unwrap();
    assert!(!details.iter().any(|site| site.span.start_byte as usize == lhs));
    let repeat = source.find("[0; 3]").unwrap();
    assert!(!details.iter().any(|site| site.span.start_byte as usize == repeat));
}
