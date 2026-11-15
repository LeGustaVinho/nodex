use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn nested_groups_expand_in_the_correct_module_without_resolving_names() {
    let source = "use a::{b::{C, D}, e}; use simple::{A, B}; use a::{b::{X as Y}, z}; mod inner { use root::{one::{two::{A}, B}, C}; }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Nested grouped imports").collect();
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "use a::{b::{C, D}, e};");
    for phrase in ["`use a::b::C;`", "`use a::b::D;`", "`use a::e;`", "existing items", "resolves", "visibility", "does not prove"] {
        assert!(items[0].text.contains(phrase), "{}", items[0].text);
    }
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    let inner_item = inner.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Nested grouped imports").unwrap();
    for path in ["root::one::two::A", "root::one::B", "root::C"] { assert!(inner_item.text.contains(path), "{}", inner_item.text); }
}
