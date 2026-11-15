use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn feature_cfg_on_match_arm_explains_both_build_configurations() {
    let source = "fn f(n: i32) { match n { #[cfg(feature = \"x\")] 0 => (), #[cfg(feature = \"y\")] 1 => (), #[cfg(target_os = \"linux\")] 2 => (), _ => () } } #[cfg(feature = \"x\")] fn g() {}";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Feature-gated match arm")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in &sites {
        assert!(source[site.span.start_byte as usize..site.span.end_byte as usize].starts_with("#[cfg(feature"));
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("included in this compilation") && text.contains("otherwise the arm is removed") && text.contains("not a runtime test") && text.contains("exhaustiveness") && text.contains("does not establish")));
    }
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("\"x\""))));
}
