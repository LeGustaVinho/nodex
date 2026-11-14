use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_borrowed_trait_object_without_mistaking_mutable_reference() {
    let text = "trait Drawable { fn draw(&self); } fn show(x: &dyn Drawable, y: &mut dyn Drawable) { let view: &dyn Drawable = x; }";
    let ir = lower_source("src/lib.rs", text);
    let show = ir.symbols.iter().find(|symbol| symbol.name == "show").unwrap();
    let facts: Vec<_> = show.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Borrowed trait object").collect();
    assert_eq!(facts.len(), 1);
    assert!(facts[0].text.contains("rather than taking ownership"));
    assert!(facts[0].text.contains("at runtime"));
    assert_eq!(&text[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "&dyn Drawable");
    let flow = ir.flows.iter().find(|flow| flow.name == "show").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Borrowed trait object" && text.contains("at runtime"))));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Shared reference type")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Dynamic trait object")));
}
