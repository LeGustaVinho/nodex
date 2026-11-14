use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn i32_literal_suffix_is_explained_in_flow_and_const_contract() {
    let source = "const N: i32 = 123i32; fn f() { let x = 123i32; let y = 123_i32; let z = 123u32; let w = 123; let neg = -123i32; }";
    let ir = lower_source("src/lib.rs", source);
    let item = ir.symbols.iter().find(|s| s.name == "N").unwrap();
    let detail = item.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Integer literal with i32 suffix").unwrap();
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "i32");
    assert!(detail.text.contains("signed 32-bit integer"));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Integer literal with i32 suffix")).collect();
    assert_eq!(sites.len(), 3, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "i32");
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        assert!(text.contains("suffix makes this literal a signed 32-bit integer"));
        assert!(text.contains("rather than performing a runtime cast"));
    }
}
