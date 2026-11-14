use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_if_else_value_and_compatible_branch_types_in_flow() {
    let source = "fn pick(flag: bool) -> i32 { let x = if flag { 1 } else { 2 }; x } fn early(flag: bool) -> i32 { if flag { 1 } else { return 2; } } fn no_else(flag: bool) { if flag { 1; } } fn pattern(x: Option<i32>) { if let Some(n) = x { consume(n); } else {} }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["pick", "early"] {
        let flow = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        let site = flow.reading.iter().flatten().find(|site| matches!(&site.content,
            ReadingContent::Detail { label, .. } if label == "Choose a value with if and else"))
            .unwrap_or_else(|| panic!("{name}: {:?}", flow.reading));
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "if");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("Only the selected branch") && text.contains("compatible result types")
                && text.contains("coercions") && text.contains("never completes") && text.contains("Boolean")));
    }
    for name in ["no_else", "pattern"] {
        let flow = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        assert!(!flow.reading.iter().flatten().any(|site| matches!(&site.content,
            ReadingContent::Detail { label, .. } if label == "Choose a value with if and else")));
    }
}



