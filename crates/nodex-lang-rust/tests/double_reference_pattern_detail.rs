use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn double_reference_pattern_distinguishes_borrow_expression_and_copies_integer() {
    let source = "fn f() { let &&x = &&42; let &&other = input; let &one = &1; let text = \"&&x\"; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content { ReadingContent::Detail { label, text, .. } if label == "Match two shared reference layers" => Some((site, text)), _ => None }).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    let (site, text) = details.iter().find(|(site, _)| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "&&x").unwrap();
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "&&x");
    for phrase in ["outer reference", "inner reference", "do not themselves create borrows", "42", "copied", "non-Copy"] { assert!(text.contains(phrase), "{text}"); }
    assert!(details.iter().any(|(_, text)| text.contains("does not establish its type")));
}
