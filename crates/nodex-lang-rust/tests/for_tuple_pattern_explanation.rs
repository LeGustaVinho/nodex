use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn tuple_for_pattern_is_explained_in_the_loop_flow() {
    let source = "fn walk(pairs: Vec<(i32, i32)>) { for (a, b) in pairs { let _ = (a, b); } for one in 0..3 { let _ = one; } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "walk").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Split each item in a for loop")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    let site = details[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "(a, b)");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("Evaluate pairs once")
        && text.contains("first element") && text.contains("inside the loop body")
        && text.contains("moved or borrowed depends")));
}
