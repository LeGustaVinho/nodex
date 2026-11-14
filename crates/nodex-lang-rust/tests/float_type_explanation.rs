use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn floating_types_are_explained_in_signatures_fields_and_local_flow() {
    let text = "struct Point { x: f32 } fn run(x: Option<f32>) -> f64 { let y: f64 = 1.0; let pair: (f32, f64) = (1.0, 2.0); y }";
    let ir = lower_source("src/lib.rs", text);
    for name in ["Point", "run"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let items = symbol.rust_contract.as_ref().unwrap();
        assert!(items.iter().any(|i| i.group == "Floating-point type" && i.text.contains("f32") && i.text.contains("significant decimal")), "{name}");
    }
    let run = ir.symbols.iter().find(|s| s.name == "run").unwrap();
    assert!(run.rust_contract.as_ref().unwrap().iter().any(|i| i.group == "Floating-point type" && i.text.contains("f64") && i.text.contains("round")));
    let flow = ir.flows.iter().find(|f| f.name == "run").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Floating-point type" && text.contains("f64"))));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Floating-point type" && text.contains("f32"))));
}
