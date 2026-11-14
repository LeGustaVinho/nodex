use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn trait_object_send_bound_is_visible_in_signatures_and_local_flow() {
    let source = "trait Drawable {} trait Send {} fn f<'a>(x: &'a (dyn Drawable + Send)) { let value: &(dyn Drawable + Send) = x; } fn plain(x: &dyn Drawable) {} fn bounds<T: Drawable + Send>() {}";
    let ir = lower_source("src/lib.rs", source);
    let function = ir.symbols.iter().find(|s| s.name == "f").unwrap();
    let item = function.rust_contract.as_ref().unwrap().iter().find(|i| i.group == "Trait object Send bound").expect("contract");
    assert!(item.text.contains("If Send resolves"));
    assert!(item.text.contains("Sync is a separate requirement"));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "Send");
    for name in ["plain", "bounds"] {
        let other = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Trait object Send bound"));
    }
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Trait object Send bound")), "{:?}", flow.reading);
    for expected in ["Dynamic trait object", "Shared reference type", "Explicit variable type"] {
        assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == expected)), "missing {expected}: {:?}", flow.reading);
    }
}
