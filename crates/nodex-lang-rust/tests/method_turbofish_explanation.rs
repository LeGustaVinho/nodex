use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_method_generic_arguments_in_flow_without_matching_plain_calls() {
    let source = "fn run(value: Value) { value.method::<u32>(7); foo::<u32>(); value.other(); value.method::<_>(8); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "run").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Explicit method generic arguments")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("value is the receiver") && text.contains("method names the method") && text.contains("<u32>") && text.contains("ordinary call arguments"))));
    assert!(sites.iter().any(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == "<_>"));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Infer a type or generic argument")));
}
