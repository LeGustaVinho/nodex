use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_boxed_trait_object_conditionally_on_name_resolution() {
    let source = "trait Drawable {} struct Holder { item: Box<dyn Drawable> } fn take(value: Box<dyn Drawable>) { let item: Box<dyn Drawable> = value; } fn ordinary(value: Box<i32>) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["item", "take"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Boxed trait object").unwrap();
        assert!(item.text.contains("runtime dispatch"));
        assert!(item.text.contains("If Box resolves"));
        assert!(item.text.contains("owns"));
        assert!(item.text.contains("does not prove"));
    }
    let ordinary = ir.symbols.iter().find(|s| s.name == "ordinary").unwrap();
    assert!(!ordinary.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Boxed trait object"));
    let flow = ir.flows.iter().find(|flow| flow.name == "take").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Boxed trait object" && text.contains("runtime dispatch"))), "{:?}", flow.reading);
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Dynamic trait object")));
}
