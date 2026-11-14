#[test]
fn exposes_generic_inherent_implementation_on_the_method() {
    let source = "struct Wrapper<T>(T); impl<T> Wrapper<T> { fn get(&self) -> &T { &self.0 } }";
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let method = ir.symbols.iter().find(|s| s.name == "get").unwrap();
    assert!(method.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Generic implementation" && item.text.contains("Wrapper<T>") && item.text.contains("family of types")));
}
