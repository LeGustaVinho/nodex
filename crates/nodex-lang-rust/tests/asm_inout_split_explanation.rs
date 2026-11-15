use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_split_inout_assembly_register_only_when_written() {
    let source = "fn f(mut x: u64) { let y: u64; unsafe { asm!(\"nop\", inout(reg) x => y); asm!(\"nop\", inout(reg) x); other!(\"nop\", inout(reg) x => y); } }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Read and write one assembly register")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "inout");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("If this invocation resolves") && text.contains("x provides the initial value") && text.contains("same register is written to y") && text.contains("target's register and type constraints") && text.contains("unsafe")));
}
