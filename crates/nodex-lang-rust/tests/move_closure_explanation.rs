use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_move_closure_without_assuming_copy_or_reference_ownership() {
    let source = "fn f(s: String, r: &i32) { let f = move |x| { use_value(&s, r, x) }; let g = |x| x; use_value(f, g); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Capture closure environment by value")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], "move");
    let ReadingContent::Detail { text, .. } = &sites[0].content else { unreachable!() };
    assert!(text.contains("Copy is copied"));
    assert!(text.contains("captures that reference"));
    assert!(text.contains("does not prove a closure can be called only once"));
}
