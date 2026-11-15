use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn ordinary_assignments_explain_writes_order_and_unit_result() {
    let source = "fn f() { x = value; obj.field = source; arr[i] = next(); *ptr = item; let limit = ..=end; (a, b) = pair; x += 1; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Assignment and unit result")).collect();
    assert_eq!(details.len(), 4, "{:?}", flow.reading);
    for item in details {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "=");
        let ReadingContent::Detail { text, .. } = &item.content else { unreachable!() };
        for phrase in ["right-hand expression first", "existing destination", "previous value is dropped", "copied or moved", "writable place", "evaluates to `()` (unit)", "not a new `let` binding"] {
            assert!(text.contains(phrase), "{text}");
        }
    }
}
