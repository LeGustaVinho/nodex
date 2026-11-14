use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn closure_result_and_general_borrow_details_can_share_one_token() {
    let source = "fn f(value: &'static i32) { let callback = || &*value; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content {
        ReadingContent::Detail { label, .. } if matches!(label.as_str(), "Borrow the result for reading" | "Create a shared borrow") => Some((site, label.as_str())),
        _ => None,
    }).collect();
    let contextual = items.iter().find(|(_, label)| *label == "Borrow the result for reading").expect("closure result explanation").0;
    let generic = items.iter().find(|(_, label)| *label == "Create a shared borrow").expect("general borrow explanation").0;
    assert_eq!(contextual.node, generic.node);
    assert_eq!(contextual.span, generic.span);
    assert_eq!(&source[contextual.span.start_byte as usize..contextual.span.end_byte as usize], "&");
}
