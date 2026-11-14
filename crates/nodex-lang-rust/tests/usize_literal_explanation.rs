use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn usize_literal_suffix_is_explained_on_the_flow() {
    let source = "fn f() { let x = 123usize; let y = 0xff_usize; let z = 123u32; let plain = 123; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Pointer-sized integer literal")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for detail in details {
        assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "usize");
        assert!(matches!(&detail.content, ReadingContent::Detail { text, .. } if text.contains("compilation target") && text.contains("must fit") && text.contains("unsigned")));
    }
}
