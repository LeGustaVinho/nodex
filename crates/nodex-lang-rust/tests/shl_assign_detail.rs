use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn shift_assignment_explains_primitive_and_custom_without_inference() {
    let source = "fn f(mut x: u32, count: u32, mut custom: Value) { x <<= count; custom <<= count; x >>= count; let s = \"<<=\"; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content { ReadingContent::Detail { label, text, .. } if label == "Shift left and assign" => Some((site, text)), _ => None }).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for (site, text) in items {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "<<=");
        for part in ["primitive integer", "ShlAssign", "does not prove", "overflow", "right side is evaluated first", "produces `()`"] { assert!(text.contains(part), "{text}"); }
    }
}
