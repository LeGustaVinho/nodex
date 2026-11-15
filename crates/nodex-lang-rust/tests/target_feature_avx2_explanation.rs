use nodex_lang_rust::lower_source;

#[test]
fn avx2_attribute_explains_call_requirements_without_assuming_cpu_support() {
    let source = "#[target_feature(enable = \"avx2\")] unsafe fn accelerated() {} #[target_feature(enable = \"avx\")] unsafe fn other() {} #[target_feature(enable = \"av x2\")] unsafe fn invalid() {}";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "accelerated").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "AVX2 target feature").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[target_feature(enable = \"avx2\")]");
    assert!(item.text.contains("AVX2 instructions") && item.text.contains("does not add AVX2 support") && item.text.contains("runtime check"));
    assert!(item.text.contains("undefined behavior") && item.text.contains("requires an unsafe call") && item.text.contains("does not prove"));
    for name in ["other", "invalid"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "AVX2 target feature"));
    }
}
