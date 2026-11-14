use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_some_ref_in_if_let_and_match_without_claiming_resolution() {
    let source = "fn f(v: Option<String>) { if let Some(ref x) = v { use_it(x); } match v { Some(ref y) => use_it(y), _ => () } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Match a Some-shaped value and borrow its field"
    )).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "(");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("shared reference") && text.contains("If Some resolves") && text.contains("does not prove")));
    }
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Bind a shared reference")));
}
