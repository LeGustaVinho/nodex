use nodex_lang_rust::lower_source;

fn items(source: &str) -> Vec<(String, String)> {
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.flavors.iter().any(|flavor| flavor == "crate-root")).unwrap();
    root.rust_contract.as_ref().into_iter().flatten().filter(|item| item.group == "Conditional crate dead-code allowance")
        .map(|item| (source[item.span.start_byte as usize..item.span.end_byte as usize].to_owned(), item.text.clone())).collect()
}

#[test]
fn inner_cfg_attr_is_conditional_and_scoped_to_the_crate() {
    let source = "#![cfg_attr(feature = \"x\", allow(dead_code))]\nfn f() {}";
    let found = items(source);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, "#![cfg_attr(feature = \"x\", allow(dead_code))]");
    for phrase in ["crate root", "feature \"x\"", "If false", "does not tell us", "runtime behavior", "forbid"] { assert!(found[0].1.contains(phrase), "missing {phrase}"); }
    assert!(items("#![cfg_attr(feature = \"x\", allow(unused))]\nfn f() {}").is_empty());
    assert!(items("#[cfg_attr(feature = \"x\", allow(dead_code))] fn f() {}").is_empty());
    assert!(items("mod m { #![cfg_attr(feature = \"x\", allow(dead_code))] fn f() {} }").is_empty());
}
