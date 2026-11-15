use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn f32_at_end_of_hex_digits_is_not_a_float_suffix() {
    let source = "fn f() { let x = 0x01_f32; let y = 0x1f32; let typed = 0x01_f32u32; let real = 1.5f32; let text = \"0x01_f32\"; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Hexadecimal digits, not an f32 suffix")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    let snippets: Vec<_> = sites.iter().map(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize]).collect();
    assert!(snippets.contains(&"0x01_f32") && snippets.contains(&"0x1f32"));
    for site in sites {
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("hexadecimal integer") && text.contains("f32 is part of the number") && text.contains("7986 in decimal") && text.contains("context") && text.contains("does not make it a floating-point value")));
    }
}
