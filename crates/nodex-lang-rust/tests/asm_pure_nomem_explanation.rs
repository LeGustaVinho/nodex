use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_pair_and_does_not_search_string_contents() {
    let source = r#"fn f() { unsafe { asm!("inc {}", inout(reg) x, options(pure, nomem)); core::arch::asm!("{}", out(reg) x, options(nomem, pure, nostack)); asm!("options(pure, nomem)"); asm!("", options(pure, readonly)); custom::asm!("", options(pure, nomem)); } }"#;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Pure assembly without outside memory")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for fact in facts {
        assert!(matches!(&fact.content, ReadingContent::Detail { text, .. } if text.contains("If this invocation resolves") && text.contains("eventually returns") && text.contains("direct input values") && text.contains("synchronize") && text.contains("non-discarded output") && text.contains("undefined behavior") && text.contains("private stack memory")));
    }
}
