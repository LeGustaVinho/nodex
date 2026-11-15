use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_direct_and_named_in_reg_without_reading_assembly_strings() {
    let source = "fn f(x: usize) { unsafe { asm!(\"/* {} */\", in(reg) x); custom::asm!(\"{v}\", v = in(reg) x); asm!(\"in(reg) hidden\"); other!(\"x\", in(reg) x); asm!(\"{}\", out(reg) x); } }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Assembly input register"
    )).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "in");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("If this invocation resolves") && text.contains("in(reg) x") && text.contains("input-only") && text.contains("depend on the target")));
    }
}
