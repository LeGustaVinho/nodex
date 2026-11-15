use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn distinguishes_raw_name_from_experimental_gen_block() {
    let source = "fn r#gen() {} fn gen() {} fn f() { let values = gen { yield 1; }; r#gen(); gen(); }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["r#gen", "gen"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Edition-sensitive gen identifier").unwrap();
        assert!(item.text.contains("2024") && item.text.contains("earlier editions") && item.text.contains("raw identifier"));
    }
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content { ReadingContent::Detail { label, text, .. } => Some((label.as_str(), text.as_str())), _ => None }).collect();
    assert!(details.iter().any(|(label, text)| *label == "Experimental gen block" && text.contains("experimental") && text.contains("iterator") && text.contains("yield") && text.contains("does not establish")), "{details:?}");
    assert!(details.iter().any(|(label, text)| *label == "Edition-sensitive gen identifier" && text.contains("raw identifier")), "{details:?}");
}
