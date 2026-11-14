use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn tuple_and_tuple_struct_positions_are_explained_without_confusing_named_fields() {
    let source = "struct Pair(i32, i32); struct Named { field: i32 } fn f(mut pair: (i32,i32), p: Pair, n: Named) { let a = pair.1; let b = p.1; pair.1 = b; let c = n.field; let d = p.0; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Select a positional field")).collect();
    assert_eq!(sites.len(), 3, "{:?}", flow.reading);
    assert!(sites.iter().filter(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "1").all(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("second") && text.contains("starts at zero") && text.contains("surrounding context"))));
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("p.1") && text.contains("second"))));
}
