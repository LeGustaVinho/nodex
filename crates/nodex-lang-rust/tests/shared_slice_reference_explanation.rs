use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn shared_slice_reference_is_explained_in_contracts_and_local_flow() {
    let source = "struct Holder<'a> { data: &'a [i32] } fn borrow<'a>(input: &'a [i32]) -> &'a [i32] { let local: &[i32] = input; local } fn mutable(input: &mut [i32]) {} fn fixed(input: &[i32; 4]) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["data", "borrow"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let detail = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Shared slice reference").unwrap();
        assert!(detail.text.contains("i32"));
        assert!(detail.text.contains("contiguous"));
        assert!(detail.text.contains("does not own"));
        assert!(detail.text.contains("cannot outlive"));
    }
    for name in ["mutable", "fixed"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Shared slice reference"));
    }
    let flow = ir.flows.iter().find(|flow| flow.name == "borrow").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Shared slice reference" && text.contains("does not own"))), "{:?}", flow.reading);
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Shared reference type")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Slice type")));
}
