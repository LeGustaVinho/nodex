use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn asm_single_place_inout_is_conditional_and_excludes_split_operands() {
    let source = r#"fn single() { unsafe { core::arch::asm!("/* {0} */", inout(reg) x); } } fn named() { asm!("/* {x} */", x = inout(reg) value); } fn split() { asm!("/* {0} */", inout(reg) input => output); } fn text() { asm!("inout(reg) x"); } fn other() { other!("", inout(reg) x); }"#;
    let file = lower_source("src/lib.rs", source);
    for name in ["single", "named"] {
        let flow = file.flows.iter().find(|flow| flow.name == name).unwrap();
        let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Possible assembly input and output")).collect();
        assert_eq!(facts.len(), 1, "{name}: {:?}", flow.reading);
        assert_eq!(&source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "inout");
        let ReadingContent::Detail { text, .. } = &facts[0].content else { unreachable!() };
        for phrase in ["If ", "standard inline assembly", "initial value", "final value back to the same place", "input => output", "depend on the target", "does not prove macro resolution"] { assert!(text.contains(phrase), "{name}: {text}"); }
    }
    for name in ["split", "text", "other"] {
        let flow = file.flows.iter().find(|flow| flow.name == name).unwrap();
        assert!(!flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Possible assembly input and output")), "{name}");
    }
}
