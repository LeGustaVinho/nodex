use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_exclusive_reference_in_declarations_and_local_flow() {
    let source = "struct Holder<'a> { value: &'a mut i32 } fn change<'a>(p: &'a mut i32, read: &i32) -> &'a mut i32 { let local: &mut i32 = p; local } fn shared(p: &i32) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["value", "change"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let contract = symbol.rust_contract.as_ref().unwrap();
        let item = contract.iter().find(|item| item.group == "Exclusive reference").unwrap();
        assert!(item.text.contains("exclusive"));
        assert!(item.text.contains("rather than taking ownership"));
        assert!(item.text.contains("other access"));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].contains("mut"));
    }
    let shared = ir.symbols.iter().find(|symbol| symbol.name == "shared").unwrap();
    assert!(!shared.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Exclusive reference"));
    let flow = ir.flows.iter().find(|flow| flow.name == "change").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Exclusive reference")), "{:?}", flow.reading);
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
}
