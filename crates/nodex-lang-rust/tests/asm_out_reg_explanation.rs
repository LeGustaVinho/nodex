use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn out_reg_output_is_explained_only_for_top_level_matching_operands() {
    let source = "fn f() { let x: u32; unsafe { core::arch::asm!(\"mov {0:e}, 1\", out(reg) x); asm!(\"out(reg) x\"); asm!(\"\", lateout(reg) y); other!(\"\", out(reg) x); } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Assembly register output")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "out");
    assert!(matches!(&item.content, ReadingContent::Detail { text, .. }
        if text.contains("If this invocation resolves") && text.contains("out(reg) x")
        && text.contains("initial value is undefined") && text.contains("may initially be uninitialized")
        && text.contains("do not prove resolution")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Macro invocation")));
}
