use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_omitted_struct_fields_in_let_and_match() {
    let source = "struct Point { x: i32, y: i32 } fn f(p: Point) -> i32 { let Point { x, .. } = p; x } fn g(p: Point) -> i32 { match p { Point { x, .. } => x } }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "g"] {
        let flow = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        let detail = flow.reading.iter().flatten().find(|site| matches!(&site.content,
            ReadingContent::Detail { label, .. } if label == "Ignore remaining fields")).unwrap_or_else(|| panic!("{name}: {:?}", flow.reading));
        assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "..");
        assert!(matches!(&detail.content, ReadingContent::Detail { text, .. } if text.contains("other fields") && text.contains("not bound") && text.contains("does not construct") && text.contains("x: x")));
    }
}
