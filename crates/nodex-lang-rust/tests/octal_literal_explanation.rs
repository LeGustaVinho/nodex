use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn octal_literal_is_explained_in_flow_and_constant_contract() {
    let source = "const MODE: u16 = 0o755; fn run() { let mode = 0o755; let typed = 0o7_55u16; let other = 755; let text = \"0o755\"; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "run").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Octal integer literal")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "0o");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("base eight") && text.contains("0 through 7") && text.contains("decimal value 493") && text.contains("does not itself apply file permissions")));
    }
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "MODE").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Octal integer literal").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "0o");
    assert!(item.text.contains("decimal value 493"));
}
