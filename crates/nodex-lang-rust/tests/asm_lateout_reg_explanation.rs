use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn lateout_reg_explains_delayed_output_conditionally_on_macro_resolution() {
    let source = r#"fn f() { unsafe { asm!("mov {0}, 1", lateout(reg) x, in(reg) y); asm!("", result = lateout(reg) dest); asm!("", out(reg) other); asm!("", lateout(xmm_reg) vector); other!(lateout(reg) z); asm!("lateout(reg) fake"); } }"#;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Late assembly register output")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for (item, name) in details.iter().zip(["x", "dest"]) {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "lateout");
        let ReadingContent::Detail { text, .. } = &item.content else { unreachable!() };
        for phrase in ["If this `asm!` resolves", "standard inline assembly macro", "chooses a register", "read all inputs before writing", "target determines", "does not prove resolution", name] {
            assert!(text.contains(phrase), "{text}");
        }
    }
}
