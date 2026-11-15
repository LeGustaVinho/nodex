use nodex_lang_rust::lower_source;
use nodex_ir::SymbolKind;

#[test]
fn explains_nested_comment_on_owning_module_and_function_without_false_positives() {
    let source = "/* outer /* inner */ outer */ fn f() { /* one /* two */ three */ use_value(); } fn plain() { /* not nested */ let s = \"/* text */\"; }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let root_items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Nested block comment").collect();
    assert_eq!(root_items.len(), 1, "{:?}", root.rust_contract);
    assert_eq!(&source[root_items[0].span.start_byte as usize..root_items[0].span.end_byte as usize], "/*");
    assert_eq!(root_items[0].span.start_byte as usize, source.find("/* inner").unwrap());
    let function = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let item = function.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Nested block comment").expect("function comment explanation");
    assert!(item.text.contains("closes only that inner level"));
    assert!(item.text.contains("still inside the outer comment"));
    assert_eq!(item.span.start_byte as usize, source.find("/* two").unwrap());
    let plain = ir.symbols.iter().find(|symbol| symbol.name == "plain").unwrap();
    assert!(!plain.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Nested block comment"));
}
