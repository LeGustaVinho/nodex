use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn slice_types_are_explained_without_confusing_fixed_arrays() {
    let source = "type Part = [i32]; fn f(xs: &[i32], fixed: &[i32; 4]) { let local: &[i32] = xs; }";
    let ir = lower_source("src/lib.rs", source);
    let alias = ir.symbols.iter().find(|s| s.name == "Part").unwrap();
    let note = alias.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Slice type").unwrap();
    assert_eq!(&source[note.span.start_byte as usize..note.span.end_byte as usize], "[i32]");
    assert!(note.text.contains("length is not fixed"));
    assert!(note.text.contains("dynamically sized"));
    assert!(note.text.contains("does not say who owns"));
    let function = ir.symbols.iter().find(|s| s.name == "f").unwrap();
    assert_eq!(function.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Slice type").count(), 1);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Slice type" && text.contains("[i32; N]"))));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
}
