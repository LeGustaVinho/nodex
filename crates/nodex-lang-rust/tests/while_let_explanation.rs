use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn while_let_explains_repeated_evaluation_and_end_without_assuming_resolution() {
    let source = "fn f(mut iter: impl Iterator<Item = i32>) { while let Some(x) = iter.next() { consume(x); } while ready() { tick(); } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Repeat while a pattern matches")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    assert!(matches!(&facts[0].content, ReadingContent::Detail { text, .. }
        if text.contains("evaluate iter.next() again") && text.contains("start the check again")
        && text.contains("first None") && text.contains("must resolve")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Paths { timing: Some(timing), .. } if timing == "Before each loop iteration")));
}
