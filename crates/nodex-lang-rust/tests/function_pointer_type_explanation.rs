use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_safe_function_pointer_type_in_contract_and_local_flow() {
    let source = "type Check = fn(i32) -> bool; type UnsafeCheck = unsafe fn(i32) -> bool; fn use_it(p: Option<fn(i32) -> bool>) { let q: fn(i32) -> bool = predicate; }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["Check", "use_it"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|i| i.group == "Function pointer type").unwrap();
        assert!(item.text.contains("(i32)") && item.text.contains("returns bool"));
        assert!(item.text.contains("not a function call or definition"));
    }
    let unsafe_alias = ir.symbols.iter().find(|s| s.name == "UnsafeCheck").unwrap();
    assert!(!unsafe_alias.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Function pointer type"));
    let flow = ir.flows.iter().find(|f| f.name == "use_it").unwrap();
    let facts = flow.reading.as_ref().unwrap();
    assert!(facts.iter().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Function pointer type" && text.contains("returns bool"))));
    assert!(facts.iter().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
}
