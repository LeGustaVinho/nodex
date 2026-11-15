use nodex_lang_rust::lower_source;

#[test]
fn outer_line_doc_explanation_is_visible_on_documented_item_only() {
    let source = "//! Crate docs\n/// Documentation\n/// More *Markdown*\n#[inline]\nfn documented() {}\n//// Ordinary comment\nfn plain() {}\n/// Structure docs\nstruct S { /// Field docs\n value: i32 }\n";
    let ir = lower_source("src/lib.rs", source);
    for name in ["documented", "S", "value"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Outer documentation comment")
            .unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "///");
        for phrase in ["following item", "documentation metadata", "Markdown", "rather than executing", "//!", "////"] {
            assert!(item.text.contains(phrase), "{name}: {}", item.text);
        }
    }
    let function = ir.symbols.iter().find(|symbol| symbol.name == "documented").unwrap();
    assert!(function.doc.as_deref().unwrap().contains("Documentation"));
    let plain = ir.symbols.iter().find(|symbol| symbol.name == "plain").unwrap();
    assert!(!plain.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Outer documentation comment"));
    let root = ir.symbols.iter().find(|symbol| symbol.name == "src").or_else(|| ir.symbols.iter().find(|symbol| symbol.parent.is_none())).unwrap();
    assert!(!root.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Outer documentation comment"));
}

