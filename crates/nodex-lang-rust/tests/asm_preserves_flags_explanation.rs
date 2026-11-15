use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn preserves_flags_is_a_conditional_target_specific_unsafe_contract() {
    let source = r#"fn direct() { unsafe { core::arch::asm!("nop", options(preserves_flags)); } } fn combined() { asm!("nop", options(nostack, preserves_flags,)); } fn text() { asm!("options(preserves_flags)"); } fn other() { other!("nop", options(preserves_flags)); } fn absent() { asm!("nop", options(nostack)); } fn nested() { asm!("nop", foo(options(preserves_flags))); }"#;
    let file = lower_source("src/lib.rs", source);
    for name in ["direct", "combined"] {
        let flow = file.flows.iter().find(|flow| flow.name == name).unwrap();
        let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Possible preserved assembly flags")).collect();
        assert_eq!(facts.len(), 1, "{name}: {:?}", flow.reading);
        assert_eq!(&source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "preserves_flags");
        let ReadingContent::Detail { text, .. } = &facts[0].content else { unreachable!() };
        for phrase in ["If ", "standard inline assembly", "original values", "restored before exit", "unsafe assembly rules", "undefined behavior", "depends on the compilation target", "different macro"] { assert!(text.contains(phrase), "{name}: {text}"); }
    }
    for name in ["text", "other", "absent", "nested"] {
        let flow = file.flows.iter().find(|flow| flow.name == name).unwrap();
        assert!(!flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Possible preserved assembly flags")), "{name}");
    }
}
