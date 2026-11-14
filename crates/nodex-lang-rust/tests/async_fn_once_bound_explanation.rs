use nodex_lang_rust::lower_source;

#[test]
fn explains_async_fn_once_generic_bound_without_guessing_trait_identity() {
    let source = "fn run<F: AsyncFnOnce(i32)>(f: F) {} fn other<F: AsyncFnMut(i32)>(f: F) {} fn none<F>(f: F) {} trait Task { fn go<F: AsyncFnOnce(i32)>(&self, f: F); }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["run", "go"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let detail = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Async callable bound").unwrap();
        assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "AsyncFnOnce(i32)");
        assert!(detail.text.contains("at most once") && detail.text.contains("produces a future") && detail.text.contains("when polled"));
        assert!(detail.text.contains("If `AsyncFnOnce` resolves") && detail.text.contains("not necessarily all consumed"));
    }
    for name in ["other", "none"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Async callable bound"));
    }
}
