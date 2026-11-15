use nodex_lang_rust::lower_source;

#[test]
fn windows_cfg_explains_target_not_host_without_claiming_active_configuration() {
    let source = "#[cfg(target_os = \"windows\")]\n#[inline]\nfn win() {}\n#[cfg(target_os = \"linux\")] fn linux() {}\n#[cfg(any(target_os = \"windows\", unix))] fn either() {}\n#[cfg(target_os = \"win dows\")] fn spaced() {}\n#[cfg_attr(target_os = \"windows\", inline)] fn indirect() {}";
    let ir = lower_source("src/lib.rs", source);
    let win = ir.symbols.iter().find(|s| s.name == "win").unwrap();
    let facts: Vec<_> = win.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Windows target build condition").collect();
    assert_eq!(facts.len(), 1, "{:?}", win.rust_contract);
    let item = facts[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[cfg(target_os = \"windows\")]");
    assert!(item.text.contains("compilation target"));
    assert!(item.text.contains("not as a runtime"));
    assert!(item.text.contains("differ from the machine running Nodex"));
    assert!(item.text.contains("does not establish"));
    for name in ["linux", "either", "spaced", "indirect"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Windows target build condition"), "{name}: {:?}", symbol.rust_contract);
    }
}
