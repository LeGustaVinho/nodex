use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_assembly_label_operands_only_in_asm_invocations() {
    let source = "fn f() { unsafe { asm!(\"jmp {}\", label { consume(); }); core::arch::asm!(\"jmp {exit}\", exit = label { unsafe { work(); } }, options(noreturn)); asm!(\"nop\"); other!(\"jmp {}\", label { work(); }); global_asm!(\"jmp {}\", label { work(); }); } }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Assembly branch to Rust block")).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "label");
        assert!(matches!(&item.content, ReadingContent::Detail { text, .. } if text.contains("If this invocation resolves") && text.contains("block's address") && text.contains("only if control reaches it") && text.contains("new safety context") && text.contains("own unsafe block") && text.contains("options(noreturn)") && text.contains("Target-specific")));
    }
}
