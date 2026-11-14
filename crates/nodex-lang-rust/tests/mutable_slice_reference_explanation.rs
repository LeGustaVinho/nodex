use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn mutable_slice_is_visible_in_signatures_fields_and_local_flows() {
    let source = "struct Holder<'a> { items: &'a mut [i32] } fn f<'a>(a: &'a mut [i32]) -> &'a mut [i32] { let x: &mut [i32] = a; x } fn other(a: &[i32], b: &mut [i32; 4]) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["items", "f"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Mutable borrowed slice").expect("slice contract");
        assert!(item.text.contains("exclusive mutable borrow"));
        assert!(item.text.contains("does not take ownership"));
        assert!(item.text.contains("number of elements"));
    }
    let other = ir.symbols.iter().find(|s| s.name == "other").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Mutable borrowed slice"));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Mutable borrowed slice" && text.contains("exclusive"))), "{:?}", flow.reading);
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Slice type")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Exclusive reference")));
}
