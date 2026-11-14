use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn trait_object_lifetime_is_explained_in_type_and_local_without_confusing_borrows() {
    let text = "type D<'a> = dyn Drawable + 'a; fn f<'a>(p: &'a (dyn Drawable + 'a)) { let x: Box<dyn Drawable + 'a> = todo!(); } fn plain<'a>(p: &'a i32) {}";
    let ir = lower_source("src/lib.rs", text);
    for name in ["D", "f"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let detail = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Trait object lifetime bound").unwrap();
        assert!(detail.text.contains("stored inside"));
        assert!(detail.text.contains("dropped earlier"));
        assert!(detail.text.contains("own borrow lifetime"));
        assert_eq!(&text[detail.span.start_byte as usize..detail.span.end_byte as usize], "'a");
    }
    let plain = ir.symbols.iter().find(|s| s.name == "plain").unwrap();
    assert!(!plain.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Trait object lifetime bound")));
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Trait object lifetime bound" && text.contains("dropped earlier"))), "{:?}", flow.reading);
}
