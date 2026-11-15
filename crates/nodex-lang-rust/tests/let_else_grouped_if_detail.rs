use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_parentheses_and_distinguishes_the_two_else_blocks() {
    let source = "fn f(flag: bool, a: Option<i32>, b: Option<i32>) { let Some(x) = (if flag { a } else { b }) else { return; }; let y = (if flag { 1 } else { 2 }); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Group an if initializer for let-else")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    assert!(matches!(&facts[0].content, ReadingContent::Detail { text, .. } if text.contains("does not permit") && text.contains("end directly in `}`") && text.contains("one branch's value first") && text.contains("must diverge") && text.contains("inner `else`") && text.contains("normal resolution")));
}
