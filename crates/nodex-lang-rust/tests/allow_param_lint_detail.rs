use nodex_lang_rust::lower_source;

#[test]
fn allow_unused_variables_on_parameter_is_explained_in_function_contract() {
    let source = "fn f(#[allow(unused_variables)] x: i32, #[allow(dead_code)] y: i32) {} #[allow(unused_variables)] fn g(z: i32) {} fn h(#[warn(unused_variables)] z: i32) {}";
    let ir = lower_source("src/lib.rs", source);
    let f = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let items: Vec<_> = f.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Allow unused parameter").collect();
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "unused_variables");
    for part in ["parameter `x`", "effective lint", "forbid", "does not prove", "runtime behavior"] { assert!(items[0].text.contains(part), "{}", items[0].text); }
    for name in ["g", "h"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Allow unused parameter"));
    }
}
