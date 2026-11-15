use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_raw_option_without_reading_strings_or_other_macros() {
    let source = "fn f() { unsafe { asm!(\"{raw}\", options(raw)); custom::asm!(\"nop\", options(nomem, raw,)); asm!(\"options(raw)\"); asm!(\"nop\", options(nomem)); other!(\"nop\", options(raw)); } }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Raw assembly template")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "raw");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("If this invocation resolves") && text.contains("braces are not interpreted") && text.contains("does not make the instructions safe")));
    }
}
