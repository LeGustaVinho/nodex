use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_shorthand_named_fields_in_match_without_confusing_construction_or_explicit_subpatterns() {
    let text = "struct Point { x: i32, y: i32 } fn f(p: Point) -> i32 { match p { Point { x, y } if x > 0 => x + y, _ => 0 } } fn g(p: Point) -> i32 { let copy = Point { x: 1, y: 2 }; match p { Point { x: 0, y: n } => n, _ => 0 } }";
    let ir = lower_source("src/lib.rs", text);
    let f = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = f.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Match and unpack named fields")).collect();
    assert_eq!(sites.len(), 1, "{:?}", f.reading);
    assert_eq!(&text[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], "{");
    assert!(matches!(&sites[0].content, ReadingContent::Detail { text, .. }
        if text.contains("x selects the x field") && text.contains("y selects the y field")
        && text.contains("x: x") && text.contains("move, copy, or borrow") && text.contains("guard")));
    let g = ir.flows.iter().find(|flow| flow.name == "g").unwrap();
    assert!(!g.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Match and unpack named fields")));
}
