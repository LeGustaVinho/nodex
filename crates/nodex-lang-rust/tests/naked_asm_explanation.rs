use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn naked_assembly_is_explained_conditionally_on_resolution() {
    let source = r##"fn sample() { core::arch::naked_asm!("ret"); custom::naked_asm!(r#"other"#); naked_asm!["x"]; core::arch::asm!("nop"); }"##;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "sample").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Naked function assembly")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in &sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "(");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("If ") && text.contains("only in a naked function") && text.contains("no function prologue or epilogue") && text.contains("calling convention") && text.contains("returns or diverges") && text.contains("Only `sym` and `const`") && text.contains("does not prove macro resolution")));
    }
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("core::arch::naked_asm") && text.contains("\"ret\""))));
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("custom::naked_asm"))));
}
