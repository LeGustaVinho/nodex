use nodex_lang_rust::lower_source;

#[test]
fn pinned_mutable_self_receiver_explains_conditional_pinning() {
    let source = "struct S; impl S { fn f(self: Pin<&mut Self>) {} fn g(self: core::pin::Pin<&'a mut Self>) {} fn shared(self: Pin<&Self>) {} fn boxed(self: Pin<Box<Self>>) {} fn arg(x: Pin<&mut Self>) {} fn normal(&mut self) {} }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "g"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Pinned mutable receiver").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("self: "));
        for phrase in ["exclusive mutable reference", "does not transfer ownership", "resolves to Rust's standard `Pin`", "not `Unpin`", "before it is dropped", "no additional movement restriction", "does not itself allocate", "does not prove"] {
            assert!(item.text.contains(phrase), "{name}: {}", item.text);
        }
    }
    for name in ["shared", "boxed", "arg", "normal"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Pinned mutable receiver"), "{name}");
    }
}
