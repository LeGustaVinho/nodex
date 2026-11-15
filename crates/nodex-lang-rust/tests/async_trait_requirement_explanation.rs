use nodex_lang_rust::lower_source;

#[test]
fn required_async_trait_method_explains_implementation_and_future() {
    let source = "trait Service { async fn run(&self); async fn result(&self) -> i32; async fn defaulted(&self) {} fn sync(&self); } async fn outside() {}";
    let ir = lower_source("src/lib.rs", source);
    for (name, output) in [("run", "()"), ("result", "i32")] {
        let method = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = method.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Required async trait method").unwrap();
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "async");
        for phrase in ["Trait Service requires", "no default body", "returns a future", "when polled", "does not schedule", "including a `&self` receiver", "not resolved"] { assert!(item.text.contains(phrase), "{name}: {}", item.text); }
        assert!(item.text.contains(&format!("return type {output}")));
    }
    for name in ["defaulted", "sync", "outside"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Required async trait method"), "{name}");
    }
}
