use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_dynamic_trait_object_in_signatures_and_local_annotations() {
    let source = "trait Drawable {} fn f(x: &dyn Drawable) { let view: &dyn Drawable = x; }";
    let ir = lower_source("src/lib.rs", source);
    let function = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let items = function.rust_contract.as_ref().unwrap();
    assert!(items.iter().any(|item| item.group == "Dynamic trait object" && item.text.contains("runtime dispatch") && item.text.contains("dynamically sized")));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites = flow.reading.as_ref().unwrap();
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Dynamic trait object" && text.contains("dyn Drawable"))), "{sites:?}");
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")), "{sites:?}");
}
