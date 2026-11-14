use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn err_pattern_explains_failure_conditionally_in_if_let_and_match() {
    let source = "fn f(r: Result<i32, i32>) { if let Err(error) = r { use_error(error); } match r { Err(error) => use_error(error), Result::Err(_) => (), Ok(_) => () } let other = Err(5); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match an Err-shaped value")).collect();
    assert_eq!(sites.len(), 3, "{:?}", flow.reading);
    for site in sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "Err");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("If Err resolves") && text.contains("error case") && text.contains("does not prove") && text.contains("does not itself throw")));
    }
}
