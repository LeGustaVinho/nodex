use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn att_syntax_option_explains_mode_target_and_name_resolution() {
    let source = "fn f() { unsafe { asm!(\"mov %eax, %eax\", options(att_syntax, nostack)); asm!(\"nop\", options(nomem)); other!(\"\", options(att_syntax)); } }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Use AT&T assembly syntax")).collect();
    assert_eq!(items.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "att_syntax");
    assert!(matches!(&items[0].content, ReadingContent::Detail { text, .. } if text.contains("If this invocation resolves") && text.contains("default Intel") && text.contains("leading %") && text.contains("only for x86") && text.contains("safety obligations")));
}
