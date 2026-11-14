use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn tuple_type_is_explained_in_contracts_and_locals() {
    let source = "struct Pair { pair: (i32, bool) } fn inspect(input: Option<(i32, bool)>) { let pair: (i32, bool) = (1, true); } fn single(x: (i32,)) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["Pair", "inspect"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Tuple type").unwrap();
        assert!(item.text.contains(".0: i32") && item.text.contains(".1: bool"));
        assert!(item.text.contains("number, order and types"));
    }
    let single = ir.symbols.iter().find(|s| s.name == "single").unwrap();
    assert!(!single.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Tuple type"));
    let flow = ir.flows.iter().find(|flow| flow.name == "inspect").unwrap();
    let facts = flow.reading.as_ref().unwrap();
    assert!(facts.iter().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Tuple type" && text.contains(".0: i32"))));
    assert!(facts.iter().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
}
