use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn grouped_callable_and_callable_field_are_distinguished_from_method_call() {
    let source = "fn f() { (func)(a); (obj.field)(a); obj.field(a); func(a); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Call a grouped expression")).collect();
    assert_eq!(facts.len(), 2, "{:?}", flow.reading);
    for (span, specific) in [("(func)", "group the expression `func`"), ("(obj.field)", "selects a field value")] {
        let site = facts.iter().find(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == span).unwrap();
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        for phrase in [specific, "following parentheses", "method call", "does not resolve", "moves or borrows"] { assert!(text.contains(phrase), "{span}: {text}"); }
    }
}
