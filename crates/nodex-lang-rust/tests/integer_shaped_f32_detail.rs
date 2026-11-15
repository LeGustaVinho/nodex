use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn integer_shaped_decimal_with_f32_suffix_is_a_float() {
    let source = "const FIVE: f32 = 5f32; fn f() { let x = 5f32; let y = 12_f32; let z = 0x01_f32; let a = 1.5f32; let b = 5i32; m!(5f32); }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|s| s.name == "FIVE").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Integer-shaped f32 literal").unwrap();
    assert!(item.text.contains("5.0") && item.text.contains("exactly"));
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Integer-shaped f32 literal")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for fact in facts {
        assert_eq!(&source[fact.span.start_byte as usize..fact.span.end_byte as usize], "f32");
        let ReadingContent::Detail { text, .. } = &fact.content else { unreachable!() };
        for phrase in ["looks like a decimal integer", "floating-point literal", "not a runtime cast", "hexadecimal digits"] { assert!(text.contains(phrase), "{text}"); }
    }
}
