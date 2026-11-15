use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn distinguishes_try_block_macro_and_raw_identifier_without_assuming_edition() {
    let source = "fn r#try() {} fn f() { let a = try { 1 }; r#try(); let b = r#try!(1); let c = try!(1); }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "r#try").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Raw identifier try").unwrap();
    assert!(item.text.contains("2015") && item.text.contains("2018") && item.text.contains("raw identifier"));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content {
        ReadingContent::Detail { label, text, .. } if matches!(label.as_str(), "Experimental try block" | "Legacy try macro spelling" | "Raw identifier try") => Some((label.as_str(), text.as_str())),
        _ => None,
    }).collect();
    assert!(details.iter().any(|(label, text)| *label == "Experimental try block" && text.contains("experimental") && text.contains("not C#-style try/catch") && text.contains("does not catch panics")), "{details:?}");
    assert!(details.iter().any(|(label, text)| *label == "Legacy try macro spelling" && text.contains("If it resolves") && text.contains("deprecated") && text.contains("r#try!")), "{details:?}");
    assert!(details.iter().any(|(label, text)| *label == "Raw identifier try" && text.contains("not the experimental")), "{details:?}");
}
