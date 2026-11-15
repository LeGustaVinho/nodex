use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn readonly_assembly_option_explains_promises_and_unsafe_obligations() {
    let source = r#"fn f() { unsafe { asm!("", options(readonly)); asm!("", options(nostack, readonly,)); asm!("", options(nomem)); other!(options(readonly)); asm!("options(readonly)"); asm!("", options(inner(readonly))); } }"#;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Read-only assembly memory")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for item in details {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "readonly");
        let ReadingContent::Detail { text, .. } = &item.content else { unreachable!() };
        for phrase in ["If this `asm!` resolves", "does not write memory accessible outside", "may read memory", "private to the assembly", "no synchronization", "undefined behavior", "does not itself make the assembly pure or safe", "does not prove"] {
            assert!(text.contains(phrase), "{text}");
        }
    }
}
