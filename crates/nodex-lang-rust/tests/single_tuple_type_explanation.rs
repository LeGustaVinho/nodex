use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_the_required_comma_in_signatures_and_local_types() {
    let source = "type One = (i32,); type Group = (i32); fn run(x: Option<(i32,)>) { let y: (i32,) = (1,); let z: (i32) = 1; }";
    let ir = lower_source("src/lib.rs", source);
    let one = ir.symbols.iter().find(|s| s.name == "One").unwrap();
    let item = one.rust_contract.as_ref().unwrap().iter().find(|i| i.group == "One-element tuple type").unwrap();
    assert!(item.text.contains("trailing comma is required"));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], ",");
    let group = ir.symbols.iter().find(|s| s.name == "Group").unwrap();
    assert!(!group.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "One-element tuple type"));
    let function = ir.symbols.iter().find(|s| s.name == "run").unwrap();
    assert!(function.rust_contract.as_ref().unwrap().iter().any(|i| i.group == "One-element tuple type"));
    let flow = ir.flows.iter().find(|f| f.name == "run").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "One-element tuple type")).collect();
    assert_eq!(sites.len(), 1);
    assert_eq!(&source[sites[0].span.start_byte as usize..sites[0].span.end_byte as usize], ",");
}
