use nodex_lang_rust::lower_source;

#[test]
fn module_inner_allow_is_attached_to_enclosing_module() {
    let source = "mod inner { #![allow(dead_code)] fn f() {} } mod other { #![allow(unused)] fn g() {} } #[allow(dead_code)] mod outer { fn h() {} }";
    let ir = lower_source("src/lib.rs", source);
    let module = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    let item = module.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Module dead-code lint allowance").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#![allow(dead_code)]");
    for phrase in ["enclosing module inner", "nested scope", "does not remove", "runtime behavior", "forbid", "rather than only the next function"] { assert!(item.text.contains(phrase), "missing {phrase}"); }
    for name in ["other", "outer", "f"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Module dead-code lint allowance"), "{name}");
    }
}
