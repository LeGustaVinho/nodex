use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_char_in_signatures_and_local_annotations_without_mistaking_strings() {
    let text = "fn f(c: char) -> Option<char> { let x: char = 'é'; let s: &str = \"char\"; Some(c) }";
    let ir = lower_source("src/lib.rs", text);
    let symbol = ir.symbols.iter().find(|s| s.name == "f").unwrap();
    let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Character type").collect();
    assert_eq!(facts.len(), 2);
    assert!(facts[0].text.contains("Unicode scalar value"));
    assert!(facts[0].text.contains("UTF-16 code unit"));
    assert!(facts[0].text.contains("multiple scalars"));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site|
        matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Unicode character type")).collect();
    assert_eq!(sites.len(), 1);
    assert_eq!(&text[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], ":");
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
}
