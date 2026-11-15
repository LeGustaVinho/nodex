use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn empty_braces_explain_possible_unit_variant_without_assuming_resolution() {
    let source = "enum E { V, Named { x: u8 } } fn f() { let a = E::V {}; let b = module::Empty {}; let c = E::Named { x: 1 }; E::V {} = a; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Possible unit enum variant construction")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for (item, name) in details.iter().zip(["V", "Empty"]) {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], name);
        let ReadingContent::Detail { text, .. } = &item.content else { unreachable!() };
        for phrase in ["If `", "resolves to an enum's unit variant", "carries no fields", "plain variant path", "empty struct", "does not establish"] {
            assert!(text.contains(phrase), "{text}");
        }
    }
}
