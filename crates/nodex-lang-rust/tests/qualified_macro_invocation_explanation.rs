use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn qualified_macro_path_and_token_input_are_explained_without_assuming_expansion() {
    let source = "module::declare!(); mod inner { module::inner!{} } fn demo(x: i32) { let _a = module::name!(x); module::sub::m![x]; plain!(x); module::name(x); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "demo").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Invoke a macro through a path")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in &sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "!");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("path resolution") && text.contains("during compilation") && text.contains("input tokens")
                && text.contains("not automatically a function argument evaluated once") && text.contains("omit or repeat")));
    }
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("module::name!") && text.contains("(x)"))));
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("module::sub::m!") && text.contains("[x]"))));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Macro invocation")), "{:?}", flow.reading);
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    let item = inner.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Qualified macro invocation").expect("module macro contract");
    assert!(item.text.contains("module::inner!") && item.text.contains("{}"));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "!");
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none()).unwrap();
    assert!(root.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Qualified macro invocation" && item.text.contains("module::declare!")));
}
