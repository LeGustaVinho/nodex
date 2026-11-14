use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_numeric_field_access_without_confusing_named_fields_or_indexes() {
    let text = "fn f(pair: (i32, i32), obj: Obj, items: &[i32]) { let first = pair.0; let second = pair.1; let named = obj.value; let indexed = items[0]; }";
    let ir = lower_source("src/lib.rs", text);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Access a positional field")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    assert!(facts.iter().any(|site| &text[site.span.start_byte as usize..site.span.end_byte as usize] == "0"));
    assert!(facts.iter().all(|site| matches!(&site.content, ReadingContent::Detail { text, .. }
        if text.contains("Positions start at zero") && text.contains("resolve the receiver's type") && text.contains("not decimal arithmetic"))));
}
