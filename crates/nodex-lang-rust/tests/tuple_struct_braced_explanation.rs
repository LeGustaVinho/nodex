use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn numeric_braced_fields_explain_construction_conditionally() {
    let source = "struct Pair(i32, i32); fn f() { let p = Pair { 0: 1, 1: 2 }; let q = Point { x: 1, y: 2 }; Pair { 0: a, 1: b } = p; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let matches: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content { ReadingContent::Detail { label, text, .. } if label == "Construct with positional fields" => Some((site, text)), _ => None }).collect();
    assert_eq!(matches.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[matches[0].0.span.start_byte as usize..matches[0].0.span.end_byte as usize], "Pair { 0: 1, 1: 2 }");
    for fragment in ["position 0 receives 1", "position 1 receives 2", "If Pair resolves", "zero-based", "visibility"] { assert!(matches[0].1.contains(fragment), "{}", matches[0].1); }
}
