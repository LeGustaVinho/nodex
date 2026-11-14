use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn tuple_patterns_are_explained_in_parameters_match_and_local_bindings() {
    let source = "fn run((x, y): (i32, i32), pair: (i32, i32)) { match pair { (a, b) => use_pair(a, b) }; let (first, second) = pair; } fn grouped((x): i32) {}";
    let ir = lower_source("src/lib.rs", source);
    let function = ir.symbols.iter().find(|symbol| symbol.name == "run").unwrap();
    let contract = function.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Tuple pattern").unwrap();
    assert!(contract.text.contains("corresponding tuple element") && contract.text.contains("one-element tuple pattern requires a comma"));
    assert_eq!(&source[contract.span.start_byte as usize..contract.span.end_byte as usize], "(x, y)");
    let flow = ir.flows.iter().find(|flow| flow.name == "run").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Match tuple positions" && text.contains("(a, b)"))));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Split a tuple into names")));
    let grouped = ir.symbols.iter().find(|symbol| symbol.name == "grouped").unwrap();
    assert!(!grouped.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Tuple pattern"));
}
