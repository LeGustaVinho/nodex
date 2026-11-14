use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn open_start_index_explains_standard_slice_conditionally_and_preserves_range_detail() {
    let text = "fn prefix(array: &[i32]) -> &[i32] { &array[..4] } fn custom(value: Custom) { let _part = &value[..4]; } fn other(array: &[i32]) { let _a = &array[1..4]; let _b = &array[..=4]; let _r = ..4; }";
    let ir = lower_source("src/lib.rs", text);
    for name in ["prefix", "custom"] {
        let flow = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        let site = flow.reading.iter().flatten().find(|site| matches!(&site.content,
            ReadingContent::Detail { label, .. } if label == "Index from the beginning up to a bound"))
            .unwrap_or_else(|| panic!("{name}: {:?}", flow.reading));
        assert_eq!(&text[site.span.start_byte as usize..site.span.end_byte as usize], "[");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("If this resolves to standard array or slice indexing")
                && text.contains("indexes 0, 1, 2, and 3") && text.contains("beyond the length panics")
                && text.contains("not an automatic owned copy") && text.contains("Other operand types")));
        assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
            ReadingContent::Detail { label, .. } if label == "Range has no start bound")), "{:?}", flow.reading);
    }
    let other = ir.flows.iter().find(|flow| flow.name == "other").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Index from the beginning up to a bound")));
}
