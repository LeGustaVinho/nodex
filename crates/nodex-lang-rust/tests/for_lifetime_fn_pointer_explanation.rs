use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_universal_lifetime_on_function_pointer_not_ordinary_fn() {
    let text = "type P = for<'a> fn(&'a i32); type Q<'a> = fn(&'a i32); fn use_it(p: for<'a> fn(&'a i32)) { let q: for<'a> fn(&'a i32) = p; }";
    let ir = lower_source("src/lib.rs", text);
    for name in ["P", "use_it"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Function pointer lifetime binder").expect(name);
        assert!(item.text.contains("every suitable choice"));
        assert!(item.text.contains("not an instruction"));
        assert_eq!(&text[item.span.start_byte as usize..item.span.end_byte as usize], "for<'a>");
    }
    let other = ir.symbols.iter().find(|symbol| symbol.name == "Q").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Function pointer lifetime binder"));
    let flow = ir.flows.iter().find(|flow| flow.name == "use_it").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Function pointer lifetime binder" && text.contains("every suitable choice"))), "{:?}", flow.reading);
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
}
