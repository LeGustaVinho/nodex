use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn grouped_arithmetic_explains_precedence_without_claiming_execution_order() {
    let source = "const C: i32 = (1 + 2) * 3; fn f() { let x = (a + b) * c; let y = c * (a - b); let z = a + b * c; let w = (a * b) * c; }";
    let ir = lower_source("src/lib.rs", source);
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "C").unwrap();
    let item = constant.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Grouped arithmetic").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "(1 + 2)");
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Grouped arithmetic")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for (site, expected) in sites.iter().zip(["(a + b)", "(a - b)"]) {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], expected);
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        for phrase in ["as one operand", "multiplication binds more tightly", "different grouping", "does not assert an execution order", "operator implementations"] { assert!(text.contains(phrase), "{text}"); }
    }
}
