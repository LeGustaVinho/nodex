use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn unsafe_function_pointer_is_explained_in_contract_and_local_flow() {
    let text = "type Callback = unsafe fn(i32) -> bool; type Safe = fn(i32) -> bool; fn run(p: unsafe fn(i32) -> bool) { let f: unsafe fn(i32) -> bool = p; }";
    let ir = lower_source("src/lib.rs", text);
    for name in ["Callback", "run"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Unsafe function pointer").unwrap();
        assert!(item.text.contains("requires an unsafe context"));
        assert!(item.text.contains("does not reveal"));
        assert_eq!(&text[item.span.start_byte as usize..item.span.end_byte as usize], "unsafe");
    }
    let safe = ir.symbols.iter().find(|symbol| symbol.name == "Safe").unwrap();
    assert!(!safe.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Unsafe function pointer"));
    let flow = ir.flows.iter().find(|flow| flow.name == "run").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Unsafe function pointer" && text.contains("caller must uphold"))));
}
