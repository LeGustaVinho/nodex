use nodex_lang_rust::lower_source;

#[test]
fn closure_mut_and_trailing_comma_are_explained_together() {
    let source = "fn f() { let closure = |mut x: i32,| { x += 1; x }; let no_comma = |mut y: i32| y; let no_mut = |z: i32,| z; }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let facts: Vec<_> = symbol.rust_contract.as_ref().into_iter().flatten().filter(|item| item.group == "Mutable closure parameter and trailing comma").collect();
    assert_eq!(facts.len(), 1, "{:?}", symbol.rust_contract);
    let item = facts[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "{ x += 1; x }");
    let text = &item.text;
    for phrase in ["x a mutable local parameter binding", "caller's binding", ": i32", "optional trailing comma", "not another parameter", "when the closure is called", "final expression", "first statement updates x", "copied or moved"] { assert!(text.contains(phrase), "{text}"); }
}
