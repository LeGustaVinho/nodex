use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn anonymous_lifetime_placeholder_is_visible_in_contract_and_local_annotation() {
    let text = "struct View<'a>(&'a str); fn f(x: &str) -> View<'_> { let y: View<'_> = View(x); y } fn named<'a>(x: &'a str) -> &'a str { x }";
    let ir = lower_source("src/lib.rs", text);
    let function = ir.symbols.iter().find(|s| s.name == "f").unwrap();
    let items: Vec<_> = function.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Inferred lifetime").collect();
    assert_eq!(items.len(), 1);
    assert_eq!(&text[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "'_");
    assert!(items[0].text.contains("compiler infers"));
    assert!(items[0].text.contains("not a named lifetime"));
    let named = ir.symbols.iter().find(|s| s.name == "named").unwrap();
    assert!(!named.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Inferred lifetime"));
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Inferred lifetime" && text.contains("borrowing"))), "{:?}", flow.reading);
}

#[test]
fn inferred_placeholder_coexists_with_trait_object_and_mutable_reference_details() {
    let text = "fn f(x: &mut i32) { let object: Box<dyn Drawable + '_> = todo!(); let r: &'_ mut i32 = x; }";
    let ir = lower_source("src/lib.rs", text);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content {
        ReadingContent::Detail { label, .. } => Some((label.as_str(), site.span.start_byte)), _ => None,
    }).collect();
    assert_eq!(details.iter().filter(|(label, _)| *label == "Inferred lifetime").count(), 2, "{details:?}");
    assert!(details.iter().any(|(label, _)| *label == "Trait object lifetime bound"), "{details:?}");
    assert!(details.iter().any(|(label, _)| *label == "Mutable reference lifetime"), "{details:?}");
}
