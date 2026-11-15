use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn discarded_asm_output_is_explained_without_claiming_resolution() {
    let source = "fn f() { unsafe { core::arch::asm!(\"\", out(reg) _); asm!(\"out(reg) _\"); asm!(\"\", out(reg) x); other!(\"\", out(reg) _); } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Discard assembly register output")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], "_");
    assert!(matches!(&sites[0].content, ReadingContent::Detail { text, .. } if text.contains("If this resolves") && text.contains("discards its final value") && text.contains("initial contents are undefined") && text.contains("does not prove macro resolution")));
    assert!(flow.reading.iter().flatten().any(|s| matches!(&s.content, ReadingContent::Detail { label, .. } if label == "Assembly register output")));
}
