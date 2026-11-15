use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn array_discard_assignment_explains_existing_destinations_and_discard() {
    let source = "fn f(array: [i32; 3]) { let mut a = 0; let mut b = 0; [a, _, b] = array; let [x, _, y] = array; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Discard an array assignment element")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "_");
    assert!(matches!(&details[0].content, ReadingContent::Detail { text, .. } if text.contains("existing destinations") && text.contains("first element to a") && text.contains("third element to b") && text.contains("not a variable") && text.contains("exits early")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Steps { label, .. } if label == "Assign parts to existing destinations")));
}
