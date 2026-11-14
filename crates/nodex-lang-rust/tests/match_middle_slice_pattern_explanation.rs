use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn middle_slice_binding_is_explained_on_the_match_flow() {
    let source = "fn inspect(values: &[i32]) { match values { [first, middle @ .., last] => { let _ = (first, middle, last); }, _ => {} } } fn bare(values: &[i32]) { match values { [first, .., last] => {}, _ => {} } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "inspect").unwrap();
    let site = flow.reading.iter().flatten().find(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Bind the middle of a sequence"))
        .unwrap_or_else(|| panic!("{:?}", flow.reading));
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "@");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("at least two elements")
        && text.contains("empty middle") && text.contains("middle @ ..") && text.contains("depends")));
    let names: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content {
        ReadingContent::Pattern { parts, .. } => Some(parts), _ => None,
    }).flatten().filter_map(|part| part.binding.as_deref()).collect();
    assert!(names.contains(&"first") && names.contains(&"middle") && names.contains(&"last"));
    let flow = ir.flows.iter().find(|flow| flow.name == "bare").unwrap();
    assert!(!flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Bind the middle of a sequence")));
}
