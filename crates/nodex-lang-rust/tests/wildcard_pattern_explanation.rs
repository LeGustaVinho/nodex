use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn nested_wildcards_explain_ignored_parts_without_confusing_identifiers_or_types() {
    let source = "fn f(input: Option<String>) { let (_, y) = (1,2); if let Some(_) = input {} match input { Some(_) => (), _ => () } let _ = input; let _item = 1; let x: Option<_> = None; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Ignore this part of the pattern")).collect();
    assert_eq!(details.len(), 3, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "_");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("without creating a variable") && text.contains("does not bind, copy, move, or borrow") && text.contains("_item")));
    }
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Ignore the result with let _")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match any remaining value")));
}
