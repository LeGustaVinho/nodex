use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn unreachable_macro_is_explained_conditionally_in_flow_and_constant() {
    let source = "const C: bool = false && unreachable!(); fn f() { unreachable!(); } fn g() { std::unreachable!(); } fn other() { custom!(); }";
    let ir = lower_source("src/lib.rs", source);
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "C").unwrap();
    let item = constant.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Conditional unreachable assertion").expect("const explanation");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "!");
    for name in ["f", "g"] {
        let flow = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        let site = flow.reading.iter().flatten().find(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Conditional unreachable assertion")).expect("flow explanation");
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "!");
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        assert!(text.contains("If it resolves to Rust's standard unreachable!"));
        assert!(text.contains("triggers a panic"));
        assert!(text.contains("does not prove that the path is impossible"));
        assert!(text.contains("same name can behave differently"));
    }
    let other = ir.flows.iter().find(|flow| flow.name == "other").unwrap();
    assert!(!other.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Conditional unreachable assertion")));
}
