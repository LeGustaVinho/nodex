use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn match_order_and_exhaustiveness_appear_on_expression_and_statement() {
    let source = "fn f(x: Option<i32>) -> i32 { let v = match x { Some(n) if n > 0 => n, _ => 0 }; match v { 0 => 1, _ => v } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Choose a matching arm")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "{");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("order written") && text.contains("guard is false") && text.contains("cover every possible value") && text.contains("supplies the value")));
    }
}
