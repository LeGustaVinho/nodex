use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_crate_root_in_import_and_expression() {
    let source = "mod inner { use crate::foo; fn run() { crate::foo(); } } fn foo() {}";
    let ir = lower_source("src/lib.rs", source);
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    assert!(inner.rust_contract.as_ref().unwrap().iter().any(|item|
        item.group == "Import from this crate" && item.text.contains("root module")));
    let flow = ir.flows.iter().find(|flow| flow.name == "run").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, text, .. }
            if label == "Path from this crate's root" && text.contains("crate::foo") && text.contains("root module"))));
}
