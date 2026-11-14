use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn f64_suffix_is_explained_in_flow_and_const_without_confusing_other_literals() {
    let source = "const X: f64 = 1.5f64; fn f() { let a = 1.5f64; let b = 1.5_f64; let c = 1.5f32; let d: f64 = 1.5; use_value(a,b,c,d); }";
    let ir = lower_source("src/lib.rs", source);
    let item = ir.symbols.iter().find(|symbol| symbol.name == "X").unwrap();
    let detail = item.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Floating-point literal with f64 suffix").expect("const explanation");
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "f64");
    assert!(detail.text.contains("1.5 itself is exact"));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Floating-point literal with f64 suffix")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "f64");
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        assert!(text.contains("64-bit floating-point"));
        assert!(text.contains("rather than performing a runtime conversion"));
    }
}
