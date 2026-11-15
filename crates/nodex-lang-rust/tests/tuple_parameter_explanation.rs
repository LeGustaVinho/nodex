use nodex_lang_rust::lower_source;

#[test]
fn tuple_parameter_destructures_one_argument_in_function_contract() {
    let source = "fn f((a, b): (i32, i32), c: i32) {} fn grouped((x): i32) {}";
    let ir = lower_source("src/lib.rs", source);
    let function = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let items: Vec<_> = function.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Tuple parameter").collect();
    assert_eq!(items.len(), 1);
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "(a, b): (i32, i32)");
    assert!(item.text.contains("one parameter") && item.text.contains("first element to a") && item.text.contains("second to b") && item.text.contains("copies or moves") && item.text.contains("irrefutable"));
    let grouped = ir.symbols.iter().find(|symbol| symbol.name == "grouped").unwrap();
    assert!(!grouped.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Tuple parameter"));
}
