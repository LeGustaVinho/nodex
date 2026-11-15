use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn grouped_alias_and_glob_are_explained_on_the_owning_module() {
    let source = "use crate::{self as current, module::*}; mod inner { use crate::{self as local, other::*}; } use crate::{self, module};";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.parent.is_none() && s.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Grouped self alias and glob import").collect();
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "{self as current, module::*}");
    for phrase in ["use crate as current;", "use crate::module::*;", "eligible visible names", "current crate's root", "Resolution, visibility and conflicts"] { assert!(items[0].text.contains(phrase), "{}", items[0].text); }
    let inner = ir.symbols.iter().find(|s| s.name == "inner").unwrap();
    assert_eq!(inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Grouped self alias and glob import").count(), 1);
}
