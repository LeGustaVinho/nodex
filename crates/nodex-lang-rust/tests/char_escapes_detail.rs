use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn simple_char_escapes_explain_each_scalar_without_matching_bytes_or_strings() {
    let source = r##"fn f() { let a = '\n'; let b = '\t'; let c = '\0'; let d = '\\'; let e = '\''; let f = '\"'; let g = '\r'; let normal = 'n'; let byte = b'\n'; let s = "\n"; m!('\n'); }"##;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Simple character escape")).collect();
    assert_eq!(facts.len(), 7, "{:?}", flow.reading);
    for (escape, code, description) in [(r"\n", "U+000A", "newline"), (r"\t", "U+0009", "tab"), (r"\0", "U+0000", "NUL"), (r"\\", "U+005C", "backslash"), (r"\'", "U+0027", "single quote"), (r#"\""#, "U+0022", "double quote"), (r"\r", "U+000D", "carriage return")] {
        let site = facts.iter().find(|site| &source[site.span.start_byte as usize..site.span.end_byte as usize] == escape).unwrap();
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        for phrase in [code, description, "one Rust `char`", "not a byte literal or a string"] { assert!(text.contains(phrase), "{escape}: {text}"); }
    }
}
