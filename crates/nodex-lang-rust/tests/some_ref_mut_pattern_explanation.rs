use nodex_ir::{Access, reading::ReadingContent};
use nodex_lang_rust::lower_source;

#[test]
fn some_ref_mut_explains_shape_and_exclusive_binding_without_assuming_option() {
    let source = "fn f(mut value: Option<i32>) { if let Some(ref mut item) = value { *item += 1; } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().collect();
    let shape = sites.iter().find(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Match a Some-shaped value")).unwrap();
    assert!(matches!(&shape.content, ReadingContent::Detail { text, .. }
        if text.contains("If Some resolves") && text.contains("only if it matches")));
    let borrow = sites.iter().find(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Bind with an exclusive reference")).unwrap();
    assert!(matches!(&borrow.content, ReadingContent::Detail { text, .. }
        if text.contains("rather than moving") && text.contains("conflicting access")));
    assert!(sites.iter().any(|site| matches!(&site.content,
        ReadingContent::Pattern { parts, .. } if parts.iter().any(|part|
            part.binding.as_deref() == Some("item") && part.access == Some(Access::BorrowMut)))));
}

#[test]
fn some_ref_mut_is_explained_in_match_arm() {
    let source = "fn f(mut value: Option<i32>) { match value { Some(ref mut item) => { *item += 1; }, _ => {} } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, text, .. } if label == "Match and mutably borrow the inner value"
            && text.contains("If Some resolves") && text.contains("instead of moving"))), "{:?}", flow.reading);
}
