use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_c_abi_pointer_in_parameter_and_local_without_guessing_symbol() {
    let source = "fn call(f: extern \"C\" fn(i32) -> i32) { let x: extern \"C\" fn(i32) -> i32 = f; let other: extern \"C-unwind\" fn(i32) -> i32 = todo!(); }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "call").unwrap();
    let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "C ABI function pointer").collect();
    assert_eq!(facts.len(), 1, "{:?}", symbol.rust_contract);
    let fact = facts[0];
    assert_eq!(&source[fact.span.start_byte as usize..fact.span.end_byte as usize], "extern \"C\"");
    assert!(fact.text.contains("function pointer type") && fact.text.contains("target platform's C calling convention") && fact.text.contains("not a function call") && fact.text.contains("does not establish"));
    let flow = ir.flows.iter().find(|flow| flow.name == "call").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "C ABI function pointer")).collect();
    assert_eq!(details.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[details[0].span.start_byte as usize..details[0].span.end_byte as usize], "extern \"C\"");
}
