use nodex_lang_rust::lower_source;

#[test]
fn explains_exact_integer_types_in_signature_without_scanning_function_body() {
    let source = "fn convert(value: u128) -> i128 { let unused: u128 = 0; 0 } fn plain(value: u64) {}";
    let ir = lower_source("src/lib.rs", source);
    let convert = ir.symbols.iter().find(|symbol| symbol.name == "convert").unwrap();
    let items: Vec<_> = convert.rust_contract.iter().flatten().filter(|item| item.group == "Integer type").collect();
    assert_eq!(items.len(), 2);
    let unsigned = items.iter().find(|item| item.text.starts_with("u128")).unwrap();
    assert!(unsigned.text.contains("0 through 2^128 - 1"));
    assert_eq!(&source[unsigned.span.start_byte as usize..unsigned.span.end_byte as usize], "u128");
    let signed = items.iter().find(|item| item.text.starts_with("i128")).unwrap();
    assert!(signed.text.contains("-2^127 through 2^127 - 1"));
    let plain = ir.symbols.iter().find(|symbol| symbol.name == "plain").unwrap();
    assert!(!plain.rust_contract.iter().flatten().any(|item| item.group == "Integer type"));
}

#[test]
fn explains_explicit_local_types_on_flow_nodes() {
    use nodex_ir::reading::ReadingContent;
    let source = "fn f() { let x: u128 = 5; let y: Option<i128> = None; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    for written in ["u128", "i128"] {
        assert!(flow.reading.iter().flatten().any(|site| {
            matches!(&site.content, ReadingContent::Detail { label, text, .. }
                    if label == "Integer type" && text.starts_with(written))
        }), "missing explanation of {written}");
    }
}
