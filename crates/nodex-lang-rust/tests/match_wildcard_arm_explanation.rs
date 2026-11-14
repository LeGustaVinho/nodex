use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn wildcard_match_arm_explains_fallback_and_guard_without_confusing_bindings() {
    let text = "fn plain(x: i32) -> i32 { match x { 0 => 1, _ => fallback() } } fn guarded(x: i32, ready: bool) -> i32 { match x { _ if ready => 1, _ => 2 } } fn nested(x: Option<i32>) -> i32 { match x { Some(_) => 1, _name => 2 } }";
    let ir = lower_source("src/lib.rs", text);
    let details = |name: &str| -> Vec<_> {
        ir.flows.iter().find(|flow| flow.name == name).unwrap().reading.iter().flatten().filter_map(|site| match &site.content {
            ReadingContent::Detail { label, text, .. } if label == "Match any remaining value" => Some((site, text)),
            _ => None,
        }).collect()
    };
    let plain = details("plain");
    assert_eq!(plain.len(), 1);
    assert_eq!(&text[plain[0].0.span.start_byte as usize..plain[0].0.span.end_byte as usize], "_");
    assert!(plain[0].1.contains("considered in order") && plain[0].1.contains("does not introduce a variable") && plain[0].1.contains("exhaustive"));
    let guarded = details("guarded");
    assert_eq!(guarded.len(), 2, "{guarded:?}");
    assert!(guarded.iter().any(|(_, text)| text.contains("guarded wildcard alone does not cover every case")));
    assert!(guarded.iter().any(|(_, text)| text.contains("unguarded _ covers every remaining value")));
    assert!(details("nested").is_empty());
}
