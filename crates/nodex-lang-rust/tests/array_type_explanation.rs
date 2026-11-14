use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_fixed_arrays_in_signatures_and_local_annotations() {
    let source = "struct Data { values: [i32; 4] } fn f(x: Option<[i32; 4]>) -> [i32; 4] { let values: [i32; 4] = [0; 4]; let nested: Option<[i32; 4]> = x; values }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["Data", "f"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(symbol.rust_contract.iter().flatten().any(|item| item.group == "Fixed-size array type" && item.text.contains("length is part of the type") && item.text.contains("i32") && item.text.contains("4")), "{name}");
    }
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites = flow.reading.as_ref().unwrap();
    assert!(sites.iter().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Fixed-size array type")).count() >= 2, "{sites:?}");
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")), "{sites:?}");
}
