use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_nop_only_for_the_exact_written_template() {
    let source = "fn f() { unsafe { core::arch::asm!(\"nop\"); ::core::arch::asm!( \"nop\", ); core::arch::asm!(\"n o p\"); core::arch::asm!(\"nop\", options(nomem)); other::arch::asm!(\"nop\"); } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Inline assembly no-operation instruction")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for fact in facts {
        assert!(matches!(&fact.content, ReadingContent::Detail { text, .. } if text.contains("If this path resolves") && text.contains("target architecture") && text.contains("unsafe context") && text.contains("other targets may reject") && text.contains("does not prove")));
    }
}
