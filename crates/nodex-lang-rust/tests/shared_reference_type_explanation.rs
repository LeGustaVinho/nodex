use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn shared_reference_types_are_explained_without_confusing_exclusive_borrows() {
    let text = "struct Holder<'a> { value: &'a i32 } fn f<'a>(x: &'a i32, y: &mut i32) -> &'a i32 { let r: &i32 = x; r }";
    let ir = lower_source("src/lib.rs", text);
    let f = ir.symbols.iter().find(|s| s.name == "f").unwrap();
    let items: Vec<_> = f.rust_contract.as_ref().unwrap().iter().filter(|i| i.group == "Shared reference type").collect();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| i.text.contains("without taking ownership") || i.text.contains("instead of taking ownership")));
    assert!(items[0].text.contains("interior mutability"));
    let field = ir.symbols.iter().find(|s| s.name == "value").unwrap();
    assert!(field.rust_contract.as_ref().unwrap().iter().any(|i| i.group == "Shared reference type"));
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Shared reference type" && text.contains("lifetime"))));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
}

#[test]
fn shared_reference_and_string_slice_details_coexist() {
    let text = "fn f(s: &str) { let copy: &str = s; }";
    let ir = lower_source("src/lib.rs", text);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    for label in ["Shared reference type", "Borrowed string slice", "String slice type", "Explicit variable type"] {
        assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label: got, .. } if got == label)), "missing {label}: {:?}", flow.reading);
    }
}
