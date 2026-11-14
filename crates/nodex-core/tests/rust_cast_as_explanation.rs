use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_cast_syntax_alongside_type_specific_conversion() {
    let source = "fn convert(p: *const u8) { let n = 511_u16 as u8; let q = p as *const (); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "convert").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Explicit type conversion")).collect();
    assert_eq!(details.len(), 2);
    assert!(details.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("511_u16") && text.contains("u8") && text.contains("specific conversions"))));
    assert!(details.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("*const ()"))));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. }
        if label.starts_with("Convert "))));
}
