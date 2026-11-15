use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn minus_and_i32_literal_are_separate_and_boundary_is_acknowledged() {
    let source = "const MIN: i32 = -2147483648i32; fn f() { let x = -123i32; let y = -1i32; let p = 123i32; let q = -123i64; let r = -variable; }";
    let ir = lower_source("src/lib.rs", source);
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "MIN").unwrap();
    let item = constant.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Negative i32 literal expression").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "-");
    assert!(item.text.contains("special literal-negation boundary"));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Negative i32 literal expression")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "-");
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        for phrase in ["unary `-` operator", "separate operator", "not part of that token", "signed 32-bit integer", "special literal-negation boundary", "overflow rules"] { assert!(text.contains(phrase), "{text}"); }
    }
}
