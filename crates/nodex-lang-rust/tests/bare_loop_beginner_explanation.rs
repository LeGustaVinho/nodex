use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn unconditional_loop_explains_repetition_exit_and_result_in_visible_flow() {
    let source = "fn choose(flag: bool) -> i32 { loop { if flag { break 7; } continue; } } fn unit() { loop { break; } } fn forever() -> ! { loop {} } fn other() { while true { break; } }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["choose", "unit", "forever"] {
        let flow = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        let site = flow.reading.iter().flatten().find(|site| matches!(&site.content,
            ReadingContent::Detail { label, .. } if label == "Repeat the loop body"))
            .unwrap_or_else(|| panic!("{name}: {:?}", flow.reading));
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "loop");
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        for expected in ["without checking a condition", "continue targeting this loop", "break targeting this loop", "supplies the result", "break without a value", "unit", "never type"] {
            assert!(text.contains(expected), "{name}: {text}");
        }
    }
    let flow = ir.flows.iter().find(|flow| flow.name == "other").unwrap();
    assert!(!flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Repeat the loop body")));
}
