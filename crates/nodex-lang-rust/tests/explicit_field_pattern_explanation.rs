use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explicit_field_patterns_explain_each_mapping_in_let_and_match() {
    let source = "struct Point { x: i32, y: i32 } fn f(p: Point, q: Point) { let Point { x: a, y: b } = p; match q { Point { x: c, y: d } => consume(c, d) } let made = Point { x: a, y: b }; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Map a named field to a pattern")).collect();
    assert_eq!(facts.len(), 4, "{:?}", flow.reading);
    for (field, binding) in [("x", "a"), ("y", "b"), ("x", "c"), ("y", "d")] {
        assert!(facts.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains(&format!("{field}: {binding}")) && text.contains("name resolution"))), "{field}: {binding}: {facts:?}");
    }
    assert!(facts.iter().all(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == ":"));
    assert!(flow.reading.iter().flatten().any(|site| matches!(site.content, ReadingContent::Pattern { .. })));
}
