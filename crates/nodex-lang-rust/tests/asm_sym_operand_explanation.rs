use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn recognizes_direct_sym_operands_without_matching_template_strings() {
    let source = r#"fn f() { unsafe { core::arch::asm!("call {}", sym function_name); asm!("/* sym fake */", named = sym other); asm!("sym fake"); custom::asm!("{}", sym nope); asm!("{}", in(reg) sym); } }"#;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Assembly symbol operand")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for fact in facts {
        assert!(matches!(&fact.content, ReadingContent::Detail { text, .. } if text.contains("If this invocation resolves") && text.contains("function or static") && text.contains("mangled symbol") && text.contains("does not call") && text.contains("unsafe context")));
    }
}
