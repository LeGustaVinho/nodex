use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn distinguishes_experimental_yield_from_raw_names_and_strings() {
    let source = "fn r#yield() {} fn f() { let x = yield 1; r#yield(); let text = \"yield\"; }";
    let file = lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|symbol| symbol.name == "r#yield").unwrap();
    assert!(symbol.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Raw identifier yield" && item.text.contains("not a yield expression")));
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content { ReadingContent::Detail { label, text, .. } if label == "Experimental yield expression" || label == "Raw identifier yield" => Some((label.as_str(), text.as_str())), _ => None }).collect();
    assert_eq!(items.len(), 2, "{items:?}");
    assert!(items.iter().any(|(label, text)| *label == "Experimental yield expression" && text.contains("suspends") && text.contains("unstable") && text.contains("reserved")));
    assert!(items.iter().any(|(label, text)| *label == "Raw identifier yield" && text.contains("not a yield expression")));
}
