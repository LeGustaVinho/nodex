use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_mutable_raw_pointer_without_confusing_const_pointer_or_reference() {
    let source = "struct Holder { p: *mut i32 } fn f(p: *mut i32, q: *const i32, r: &mut i32) -> *mut i32 { let x: *mut i32 = p; x } fn other(p: *const i32) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["p", "f"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let explanations: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Mutable raw pointer").collect();
        assert!(!explanations.is_empty(), "{name}");
        for item in explanations {
            let span = &source[item.span.start_byte as usize..item.span.end_byte as usize];
            assert!(span.starts_with("*mut"), "{span}");
            assert!(item.text.contains("null or dangling"));
            assert!(item.text.contains("dereferencing it requires unsafe"));
            assert!(item.text.contains("does not own"));
        }
    }
    let other = ir.symbols.iter().find(|symbol| symbol.name == "other").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Mutable raw pointer"));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Mutable raw pointer" && text.contains("aliasing rules"))), "{:?}", flow.reading);
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
}
