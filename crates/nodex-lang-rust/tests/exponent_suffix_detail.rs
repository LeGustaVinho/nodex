use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn scientific_notation_and_f64_suffix_are_visible_in_flow_and_const() {
    let source = "const C: f64 = 1e-3f64; fn f() { let x = 1e-3f64; let y = 2E+4_f64; let z = 1e-3f32; let n = 1.5f64; let text = \"1e-3f64\"; m!(1e-3f64); }";
    let ir = lower_source("src/lib.rs", source);
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "C").unwrap();
    let item = constant.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Scientific notation with f64 suffix").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "e-3");
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Scientific notation with f64 suffix")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for (site, exponent) in sites.iter().zip(["e-3", "E+4"]) {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], exponent);
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        for phrase in ["scientific notation", "multiplied by 10", "negative exponent", "positive exponent", "`f64` suffix", "not a runtime cast", "rounding"] { assert!(text.contains(phrase), "{text}"); }
    }
}
