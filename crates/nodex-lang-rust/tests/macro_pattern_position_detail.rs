use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn distinguishes_pattern_macro_from_expression_macro_and_guard() {
    let source = "fn f(x: i32) { match x { pat!() => (), _ if guard!() => (), _ => () }; let pat!() = x; pat!(); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Macro in a pattern")).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "pat!()");
        assert!(matches!(&item.content, ReadingContent::Detail { text, .. } if text.contains("where Rust expects a pattern") && text.contains("macro expands") && text.contains("do not themselves reveal") && text.contains("Name resolution") && text.contains("irrefutable")));
    }
}
