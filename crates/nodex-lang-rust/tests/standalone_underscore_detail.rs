use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn underscore_distinguishes_discard_pattern_and_inference() {
    let source = "fn f() { _ = work(); let _ = work(); let x: _ = 1; let _value = 2; match x { _ => () }; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Discard an assignment result")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], "_");
    assert!(matches!(&sites[0].content, ReadingContent::Detail { text, .. } if text.contains("evaluated") && text.contains("not bound") && text.contains("pattern") && text.contains("infer") && text.contains("_value")));
    assert!(flow.reading.iter().flatten().any(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Ignore the result with let _")));
    assert!(flow.reading.iter().flatten().any(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Infer a type or generic argument")));
    assert!(flow.reading.iter().flatten().any(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Match any remaining value")));
}
