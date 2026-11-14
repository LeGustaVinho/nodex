use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_minimum_length_and_ignored_middle_in_match_and_if_let() {
    let source = "fn f(xs: &[i32]) { match xs { [first, .., last] => use_it(first, last), _ => () } if let [a, .., b] = xs { use_it(a,b); } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Match sequence endpoints"
    )).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "..");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. }
            if text.contains("at least 2 elements") && text.contains("including none") && text.contains("too short") && text.contains("does not establish ownership")));
    }
}
