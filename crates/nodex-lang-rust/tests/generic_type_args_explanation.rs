use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn mixed_lifetime_type_and_constant_arguments_are_explained() {
    let source = "fn f<'a, T>(x: Type<'a, T, 3>) { let value: Type<'a, T, 3> = x; let other: Vec<T> = todo!(); }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Mixed generic type arguments").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "Type<'a, T, 3>");
    for expected in ["lifetime argument", "type argument", "constant integer argument", "does not by itself extend", "necessarily being a length", "does not construct", "does not establish"] { assert!(item.text.contains(expected), "missing {expected}"); }
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Mixed generic type arguments")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
}
