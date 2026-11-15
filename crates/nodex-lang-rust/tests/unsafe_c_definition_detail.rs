use nodex_lang_rust::lower_source;

#[test]
fn unsafe_c_definition_combines_abi_and_caller_obligations() {
    let source = "unsafe extern \"C\" fn f() {}";
    let file = lower_source("src/lib.rs", source);
    let f = file.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let items = f.rust_contract.as_ref().unwrap();
    let abi = items.iter().find(|item| item.group == "C calling convention").unwrap();
    let safety = items.iter().find(|item| item.group == "Unsafe function").unwrap();
    assert!(abi.text.contains("C application binary interface") && abi.text.contains("does not export an unmangled symbol"));
    assert!(safety.text.contains("caller must") && safety.text.contains("unsafe context") && safety.text.contains("cannot be inferred"));
}
