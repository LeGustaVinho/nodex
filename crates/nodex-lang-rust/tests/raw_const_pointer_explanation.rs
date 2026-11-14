use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn raw_const_pointer_is_explained_on_contracts_and_local_flow() {
    let source = "struct S { p: *const i32 } fn f(p: *const i32) -> *const i32 { let x: *const i32 = p; x } fn mutable(p: *mut i32) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["p", "f"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let details: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Raw const pointer").collect();
        assert!(!details.is_empty(), "{name}");
        assert!(details.iter().all(|item| item.text.contains("null, dangling, or misaligned") && item.text.contains("requires unsafe") && item.text.contains("does not by itself make mutation legal")));
        assert!(details.iter().all(|item| source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("*const")));
    }
    let mutable = ir.symbols.iter().find(|symbol| symbol.name == "mutable").unwrap();
    assert!(!mutable.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Raw const pointer"));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Raw const pointer")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
}
