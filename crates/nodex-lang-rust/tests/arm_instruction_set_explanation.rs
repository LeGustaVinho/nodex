use nodex_lang_rust::lower_source;

#[test]
fn direct_arm_a32_attribute_explains_codegen_and_target_requirements() {
    let source = "#[instruction_set(arm::a32)] fn arm_code() {} struct S; impl S { #[instruction_set( arm :: a32 )] fn method(&self) {} } #[instruction_set(arm::t32)] fn thumb() {} #[cfg_attr(target_arch = \"arm\", instruction_set(arm::a32))] fn indirect() {} #[instruction_set(arm::a32x)] fn other() {} trait T { #[instruction_set(arm::a32)] fn required(); }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["arm_code", "method"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "ARM A32 instruction set").collect();
        assert_eq!(facts.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = facts[0];
        let written = &source[item.span.start_byte as usize..item.span.end_byte as usize];
        assert!(written.starts_with("#[instruction_set("));
        assert!(item.text.contains("ARM A32 instruction set"));
        assert!(item.text.contains("target and toolchain must support"));
        assert!(item.text.contains("not a runtime CPU check"));
        assert!(item.text.contains("does not establish which target"));
        assert!(item.text.contains("Inline assembly"));
    }
    for name in ["thumb", "indirect", "other", "required"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "ARM A32 instruction set"), "{name}: {:?}", symbol.rust_contract);
    }
}
