use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn hexadecimal_escapes_in_text_and_byte_strings_are_distinguished() {
    let source = r##"const A: &str = "\x41"; static B: &[u8] = b"\xFF"; fn f() { let a = "\x41"; let b = b"\xFF"; let escaped = "\\x41"; let raw = r"\x41"; let byte_raw = br"\xFF"; let c = '\x41'; }"##;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Hex escape in a string")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for (name, expected, description) in [("A", r"\x41", "ordinary text string"), ("B", r"\xFF", "byte string")] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Hex escape in a string").unwrap();
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], expected);
        assert!(item.text.contains(description) && item.text.contains("exactly two hexadecimal digits"));
        assert!(sites.iter().any(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == expected && matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains(description))));
    }
}
