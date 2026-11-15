use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn xor_assignment_explains_builtin_types_and_custom_implementation() {
    let source = "fn f(mut place: i32, value: i32) { place ^= value; place |= value; let _ = place ^ value; } fn flag(mut a: bool, b: bool) { a ^= b; }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "flag"] {
        let flow = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "XOR and assign")).collect();
        assert_eq!(sites.len(), 1, "{name}: {:?}", flow.reading);
        let site = sites[0];
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "^=");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("mutable place") && text.contains("unit value") && text.contains("bitwise exclusive OR") && text.contains("booleans") && text.contains("BitXorAssign") && text.contains("does not establish")));
    }
}
