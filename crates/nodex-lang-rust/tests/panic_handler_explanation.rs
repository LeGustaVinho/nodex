use nodex_lang_rust::lower_source;

#[test]
fn panic_handler_explains_signature_and_context_without_claiming_validity() {
    let source = "#![no_std] use core::panic::PanicInfo; #[panic_handler] #[inline(never)] fn panic(info: &PanicInfo) -> ! { loop {} } #[panic_handler] fn invalid() {} #[cfg_attr(test, panic_handler)] fn conditional() {} fn plain() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["panic", "invalid"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Panic handler").collect();
        assert_eq!(items.len(), 1, "{}: {:?}", name, symbol.rust_contract);
        assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "#[panic_handler]");
        assert!(items[0].text.contains("fn(&PanicInfo) -> !") && items[0].text.contains("cannot return normally") && items[0].text.contains("exactly one") && items[0].text.contains("no_std") && items[0].text.contains("does not prove"));
    }
    for name in ["conditional", "plain"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Panic handler"));
    }
}
