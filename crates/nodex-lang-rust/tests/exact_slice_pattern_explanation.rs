use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn exact_sequence_pattern_explains_length_and_positions_in_flow() {
    let source = "fn f(s: &[i32]) { if let [a, b, c] = s { consume(a); } match s { [x, y, z] => (), [first, rest @ ..] => (), _ => () } let [a,b,c] = [1,2,3]; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Match an exact-length sequence")).collect();
    assert_eq!(details.len(), 3, "{:?}", flow.reading);
    for site in details {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "[");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("exactly 3") && text.contains("index 0") && text.contains("different length") && text.contains("rejected during compilation")));
    }
}
