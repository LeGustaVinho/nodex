use nodex_lang_rust::lower_source;
use nodex_ir::reading::ReadingContent;

#[test]
fn explains_str_in_signature_and_local_annotations() {
    let text = "fn greet(text: &str, nested: Option<&str>) -> &str { let copy: &str = text; copy } fn number(x: i32) {}";
    let ir = lower_source("src/lib.rs", text);
    let greet = ir.symbols.iter().find(|s| s.name == "greet").unwrap();
    let notes: Vec<_> = greet.rust_contract.as_ref().unwrap().iter().filter(|i| i.group == "String slice type").collect();
    assert_eq!(notes.len(), 3);
    assert!(notes[0].text.contains("valid UTF-8"));
    assert!(notes[0].text.contains("dynamically sized"));
    assert!(notes[0].text.contains("String, which owns"));
    assert_eq!(&text[notes[0].span.start_byte as usize..notes[0].span.end_byte as usize], "str");
    let flow = ir.flows.iter().find(|f| f.name == "greet").unwrap();
    let local_offset = text.find("let copy: &str").unwrap() + "let copy: &".len();
    assert!(flow.reading.iter().flatten().any(|site| site.span.start_byte as usize == local_offset && matches!(&site.content, ReadingContent::Detail { label, .. } if label == "String slice type")));
    let other = ir.symbols.iter().find(|s| s.name == "number").unwrap();
    assert!(!other.rust_contract.as_ref().is_some_and(|items| items.iter().any(|i| i.group == "String slice type")));
}
