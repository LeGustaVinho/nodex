use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_inlateout_same_place_without_assuming_asm_macro_identity() {
    let source = "macro_rules! asm { ($($t:tt)*) => { () } } fn f(mut x: usize, mut y: usize) { unsafe { asm!(\"\", inlateout(reg) x); asm!(\"\", inlateout(reg) x => y); asm!(\"\", inout(reg) x); other!(\"\", inlateout(reg) x); } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Late input/output register")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "inlateout");
    assert!(matches!(&details[0].content, ReadingContent::Detail { text, .. } if text.contains("If this invocation resolves") && text.contains("same mutable place") && text.contains("reuse a register") && text.contains("before all potentially overlapping inputs") && text.contains("does not prove")));
}
