use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn recovered_invalid_numbers_are_explained_without_scanning_text() {
    let source = "fn f() { let a = 0b0102; let b = 0o1279; let c = 2e; let good = (0b010, 0o127, 2e3); let s = \"0b0102 0o1279 2e\"; /* 0b0102 */ }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Invalid numeric literal")).collect();
    assert_eq!(sites.len(), 3, "{:?}", flow.reading);
    for (literal, expected) in [("0b0102", "digit 2"), ("0o1279", "digit 9"), ("2e", "missing exponent digits")] {
        let site = sites.iter().find(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == literal).unwrap_or_else(|| panic!("missing {literal}: {sites:?}"));
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains(expected) && text.contains("not complete validation")));
    }
    assert!(!file.diagnostics.is_empty(), "parser warning must remain available");
}
