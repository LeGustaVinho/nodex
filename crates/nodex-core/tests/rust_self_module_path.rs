use nodex_ir::reading::ReadingContent;

#[test]
fn explains_self_paths_in_import_and_expression() {
    let source = "mod inner { use self::foo; fn run() { self::foo(); } fn foo() {} }";
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let module = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    assert!(module.rust_contract.as_ref().unwrap().iter().any(|item|
        item.group == "Current module path" && item.text.contains("current module")));
    let flow = ir.flows.iter().find(|flow| flow.name == "run").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, text, .. }
        if label == "Name in the current module" && text.contains("self::foo"))));
}
