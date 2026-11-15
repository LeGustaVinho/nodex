use nodex_lang_rust::lower_source;

fn facts(source: &str) -> Vec<(String, String)> {
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.flavors.iter().any(|flavor| flavor == "crate-root")).unwrap();
    root.rust_contract.as_ref().into_iter().flatten().filter(|item| item.group == "Unstable feature gates").map(|item| (source[item.span.start_byte as usize..item.span.end_byte as usize].into(), item.text.clone())).collect()
}

#[test]
fn feature_gates_keep_names_and_explain_nightly_and_uncertainty() {
    let items = facts("#![feature(generic_const_exprs, /* explanation */ never_type,)]\nfn f() {}");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "#![feature(generic_const_exprs, /* explanation */ never_type,)]");
    for phrase in ["generic_const_exprs, never_type", "nightly compiler", "change or be removed", "does not establish", "crate root"] { assert!(items[0].1.contains(phrase), "{}", items[0].1); }
    assert!(facts("#![cfg_attr(test, feature(never_type))]\nfn f() {}").is_empty());
    assert!(facts("mod nested { #![feature(never_type)] fn f() {} }").is_empty());
    assert!(facts("#![feature_flag(never_type)]\nfn f() {}").is_empty());
}
