use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_noreturn_with_and_without_label_without_assuming_macro_identity() {
    let source = "macro_rules! asm { ($($t:tt)*) => { () } } fn f() { unsafe { asm!(\"\", options(noreturn)); asm!(\"\", label { }, options(nostack, noreturn)); asm!(\"\", options(nostack)); other!(\"\", options(noreturn)); } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Assembly does not fall through")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "noreturn");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("If this invocation resolves") && text.contains("undefined behavior") && text.contains("`label` block") && text.contains("can return `()`") && text.contains("not dropped") && text.contains("does not establish")));
    }
}
