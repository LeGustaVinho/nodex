use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_nostack_option_but_not_strings_nested_operands_or_other_macros() {
    let source = "fn f(x: usize) { unsafe { asm!(\"nop\", options(nostack, nomem)); custom::asm!(\"nop\", options(nostack)); asm!(\"options(nostack)\"); asm!(\"{}\", in(reg) options(nostack)); other!(\"nop\", options(nostack)); asm!(\"nop\", options(nostackish)); } }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Assembly nostack promise"
    )).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "nostack");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("If this invocation resolves") && text.contains("red zone") && text.contains("unsafe obligation") && text.contains("undefined behavior") && text.contains("alignment")));
    }
}
