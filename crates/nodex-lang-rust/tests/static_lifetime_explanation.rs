use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn static_lifetime_explains_references_and_bounds_without_promising_value_lifetime() {
    let source = "fn reference(x: &'static str) -> &'static str { let local: &'static str = x; local } fn bound<T: 'static>(x: T) {} fn ordinary<'a>(x: &'a str) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["reference", "bound"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let detail = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Static lifetime").unwrap();
        assert!(detail.text.contains("no borrowed data with a shorter lifetime"));
        assert!(detail.text.contains("dropped earlier"));
        assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "'static");
    }
    let symbol = ir.symbols.iter().find(|s| s.name == "ordinary").unwrap();
    assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Static lifetime"));
    let flow = ir.flows.iter().find(|f| f.name == "reference").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Static lifetime" && text.contains("dropped earlier"))), "{:?}", flow.reading);
}

#[test]
fn static_and_other_lifetime_details_coexist() {
    let source = "trait Draw {} fn f(x: &'static i32) { let y: &'static i32 = x; let z: Box<dyn Draw + 'static> = todo!(); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let sites = flow.reading.as_ref().unwrap();
    for label in ["Static lifetime", "Trait object lifetime bound"] {
        assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Detail { label: found, .. } if found == label)), "Missing {label}: {sites:?}");
    }
    assert!(sites.iter().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Static lifetime")).count() >= 2, "{sites:?}");
}
