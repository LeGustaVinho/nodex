use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_named_lifetime_on_shared_references_in_contract_and_flow() {
    let text = "struct Holder<'a> { v: &'a i32 } fn f<'a>(x: &'a i32, y: &'a mut i32) -> &'a i32 { let r: &'a i32 = x; r }";
    let ir = lower_source("src/lib.rs", text);
    let f = ir.symbols.iter().find(|s| s.name == "f").unwrap();
    let items: Vec<_> = f.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Named reference lifetime").collect();
    assert_eq!(items.len(), 2, "{items:?}");
    assert!(items.iter().all(|item| item.text.contains("does not extend")));
    assert!(items.iter().all(|item| &text[item.span.start_byte as usize..item.span.end_byte as usize] == "'a"));
    let field = ir.symbols.iter().find(|s| s.name == "v").unwrap();
    assert!(field.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Named reference lifetime"));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    for expected in ["Named reference lifetime", "Shared reference type", "Explicit variable type"] {
        assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == expected)), "{expected}: {:?}", flow.reading);
    }
}
