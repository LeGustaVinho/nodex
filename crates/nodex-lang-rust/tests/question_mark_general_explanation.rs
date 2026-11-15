use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn question_mark_explains_generic_propagation_alongside_existing_paths() {
    let source = "fn f(value: Result<i32, E>) -> Result<i32, E> { let x = value?; Ok(x) } fn g(other: Option<i32>) -> Option<i32> { let y = other /* checked */ ?; Some(y) }";
    let ir = lower_source("src/lib.rs", source);
    for (name, operand) in [("f", "value"), ("g", "other")] {
        let flow = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        let details: Vec<_> = flow
            .reading
            .iter()
            .flatten()
            .filter(|site| {
                matches!(&site.content,
            ReadingContent::Detail { label, .. } if label == "Continue or propagate with ?")
            })
            .collect();
        assert_eq!(details.len(), 1, "{name}: {:?}", flow.reading);
        let site = details[0];
        assert_eq!(
            &source[site.span.start_byte as usize..site.span.end_byte as usize],
            "?"
        );
        let ReadingContent::Detail { text, .. } = &site.content else {
            unreachable!()
        };
        assert!(text.contains(&format!("Evaluate {operand} once")));
        assert!(text.contains("type is not established by syntax alone"));
        assert!(text.contains("closure, async block, or try block"));
        assert_eq!(
            flow.reading.iter().flatten().filter(|paths| paths.span == site.span &&
                matches!(&paths.content, ReadingContent::Paths { branches, .. } if branches.len() == 2)).count(),
            1,
            "{name}: {:?}", flow.reading,
        );
    }
}
