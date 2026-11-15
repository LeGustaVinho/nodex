use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn nomem_option_is_explained_in_asm_only_and_can_coexist_with_other_options() {
    let source = "fn f() { unsafe { core::arch::asm!(\"nop\", options(nomem)); asm!(\"nop\", options(nomem, preserves_flags)); other!(options(nomem)); asm!(\"options(nomem)\"); asm!(\"nop\", options(nostack)); } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Assembly does not access outside memory")).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "nomem");
        assert!(matches!(&item.content, ReadingContent::Detail { text, .. }
            if text.contains("If this invocation resolves") && text.contains("memory accessible outside")
            && text.contains("synchronize") && text.contains("undefined behavior")
            && text.contains("does not say the assembly has no other effects") && text.contains("nostack")));
    }
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Macro invocation")));
}
