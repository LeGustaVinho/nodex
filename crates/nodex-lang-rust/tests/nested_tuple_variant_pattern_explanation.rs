use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn nested_tuple_pattern_explains_layers_without_assuming_option_identity() {
    let source = "enum Local { Some((i32, i32)), Other } fn f(value: Local) -> i32 { match value { Local::Some((a, b)) => a + b, _ => 0 } } fn opt(value: Option<(i32,i32)>) -> i32 { match value { Some((a,b)) => a + b, _ => 0 } } fn other(value: Option<(i32,i32)>) { let _v = Some((1,2)); match value { Some(pair) => consume(pair), _ => () } }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "opt"] {
        let flow = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
            ReadingContent::Detail { label, .. } if label == "Match a nested tuple pattern")).collect();
        assert_eq!(sites.len(), 1, "{name}: {:?}", flow.reading);
        assert_eq!(&source[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], ",");
        assert!(matches!(&sites[0].content, ReadingContent::Detail { text, .. }
            if text.contains("a matches tuple position .0") && text.contains("b matches tuple position .1")
                && text.contains("names can instead resolve to constants") && text.contains("spelling alone is not proof")
                && text.contains("moved, copied, or borrowed")));
    }
    let other = ir.flows.iter().find(|flow| flow.name == "other").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Match a nested tuple pattern")));
}
