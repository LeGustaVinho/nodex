use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_explicit_type_argument_in_call_flow() {
    let text = "fn run() { foo::<u32>(); foo::<_>(); foo::<42>(); foo(); }";
    let ir = lower_source("src/lib.rs", text);
    let flow = ir.flows.iter().find(|f| f.name == "run").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit generic type argument")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    assert_eq!(&text[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "<u32>");
    assert!(matches!(&facts[0].content, ReadingContent::Detail { text, .. } if text.contains("turbofish") && text.contains("infer") && text.contains("constraints")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Infer a type or generic argument")));
}
