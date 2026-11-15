use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_const_operands_without_interpreting_template_strings() {
    let source = "fn f() { unsafe { asm!(\"{}\", const N); custom::asm!(\"{v}\", v = const SIZE); asm!(\"const HIDDEN\"); other!(\"{}\", const N); asm!(\"{}\", in(reg) N); } }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Assembly constant operand")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "const");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("If this invocation resolves") && text.contains("compile-time") && text.contains("inserted as text") && text.contains("Another macro")));
    }
}
