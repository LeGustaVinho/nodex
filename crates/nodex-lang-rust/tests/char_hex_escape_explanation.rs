use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn hexadecimal_escape_is_explained_as_a_char_not_a_byte() {
    let source = r"fn f() { let character = '\x41'; let ordinary = 'A'; let byte = b'\x41'; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Hexadecimal character escape")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let site = sites[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], r"\x41");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("exactly two hexadecimal digits")
        && text.contains("U+0041") && text.contains("`A`") && text.contains("00 through 7F")
        && text.contains("not a byte literal")));
}
