use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn initial_bar_is_explained_only_when_present() {
    let source = "enum E { A, B } fn f(x: E) { match x { | E::A | E::B => (), _ => () }; match x { E::A | E::B => (), _ => () }; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let matches: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content { ReadingContent::Detail { label, text, .. } if label == "Initial separator in a match pattern" => Some((site, text)), _ => None }).collect();
    assert_eq!(matches.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[matches[0].0.span.start_byte as usize..matches[0].0.span.end_byte as usize], "|");
    assert_eq!(matches[0].0.span.start_byte as usize, source.find("| E::A").unwrap());
    for phrase in ["optional", "does not add an empty alternative", "same names", "binding modes"] { assert!(matches[0].1.contains(phrase)); }
}
