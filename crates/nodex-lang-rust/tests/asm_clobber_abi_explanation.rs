use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;
#[test]
fn c_abi_clobber_is_conditional_and_only_found_in_asm_tokens() {
    let source = r##"fn f() { unsafe { core::arch::asm!("call {}", sym foo, clobber_abi("C")); custom::asm!("", clobber_abi("C", "system")); asm!("clobber_abi(\"C\")"); asm!("", clobber_abi("system")); other!(clobber_abi("C")); asm!("", foo(clobber_abi("C"))); } }"##;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Assembly ABI register clobbers")).collect();
    assert_eq!(sites.len(), 2, "{:?}", flow.reading);
    for site in &sites {
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "clobber_abi");
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("If ") && text.contains("not fully preserved") && text.contains("implicit late output") && text.contains("explicit register output takes precedence") && text.contains("exact set depends on the target") && text.contains("does not itself make the call") && text.contains("outputs must name explicit registers") && text.contains("does not prove macro resolution")));
    }
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("custom::asm"))));
}
