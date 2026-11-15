use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn direct_ref_mut_let_pattern_explains_exclusive_reference() {
    let source = "fn f() { let mut value = 1; let ref mut x = value; *x += 1; } fn shared() { let value = 1; let ref x = value; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Bind with an exclusive reference")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let item = sites[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "ref");
    let ReadingContent::Detail { text, .. } = &item.content else { unreachable!() };
    for phrase in ["ref mut x", "exclusive mutable reference", "rather than moving", "dereference", "conflicting access", "cannot outlive"] { assert!(text.contains(phrase), "{text}"); }
    let shared = ir.flows.iter().find(|flow| flow.name == "shared").unwrap();
    assert!(!shared.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Bind with an exclusive reference")));
}
