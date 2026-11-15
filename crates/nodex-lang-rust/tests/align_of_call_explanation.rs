use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_alignment_conditionally_in_flow_and_constant() {
    let source = "const A: usize = std::mem::align_of::<u32>(); fn f<T>() { let a = std::mem::align_of::<T>(); let b = core::mem::align_of::<u64>(); let c = custom::mem::align_of::<T>(); let d = std::mem::align_of_val(&a); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Query a type's alignment")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for fact in facts {
        assert!(matches!(&fact.content, ReadingContent::Detail { text, .. } if text.contains("If this path resolves") && text.contains("minimum alignment") && text.contains("positive power of two") && text.contains("compilation target") && text.contains("not its size")));
    }
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "A").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Query a type's alignment").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "std::mem::align_of");
    assert!(item.text.contains("u32"));
}
