use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn mutable_reference_patterns_are_explained_without_confusing_ref_mut() {
    let source = "fn f(r: &mut i32) { let &mut x = r; if let &mut y = r { consume(y); } match r { &mut z => consume(z) } let ref mut w = 1; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match through a mutable reference")).collect();
    assert_eq!(details.len(), 3, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "&");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("binding by value") && text.contains("ref mut x") && text.contains("moving a non-Copy")));
    }
}
