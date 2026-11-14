use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_named_mutable_reference_lifetime_in_contract_and_local_flow() {
    let source = "struct Holder<'a> { value: &'a mut i32 } fn change<'a>(x: &'a mut i32) -> &'a mut i32 { let y: &'a mut i32 = x; y } fn shared<'a>(x: &'a i32) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["value", "change"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Mutable reference lifetime").unwrap();
        assert!(item.text.contains("exclusive borrow"));
        assert!(item.text.contains("not a runtime timer"));
        assert!(item.text.contains("does not extend"));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "'a");
    }
    let shared = ir.symbols.iter().find(|symbol| symbol.name == "shared").unwrap();
    assert!(!shared.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Mutable reference lifetime"));
    let flow = ir.flows.iter().find(|flow| flow.name == "change").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Mutable reference lifetime")), "{:?}", flow.reading);
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Exclusive reference")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
}
