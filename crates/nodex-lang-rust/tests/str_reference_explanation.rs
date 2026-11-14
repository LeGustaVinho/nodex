use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn borrowed_str_is_explained_on_signatures_fields_and_locals() {
    let source = "struct Holder<'a> { value: &'a str } fn read<'a>(s: &'a str) -> &'a str { let x: &str = s; x } fn unrelated(s: &mut str) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["read", "value"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Borrowed string slice").unwrap();
        assert!(item.text.contains("UTF-8"));
        assert!(item.text.contains("without taking ownership"));
        assert!(item.text.contains("must not outlive"));
    }
    let other = ir.symbols.iter().find(|symbol| symbol.name == "unrelated").unwrap();
    assert!(!other.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Borrowed string slice")));
    let flow = ir.flows.iter().find(|flow| flow.name == "read").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Borrowed string slice" && text.contains("UTF-8"))), "{:?}", flow.reading);
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "String slice type")));
}
