use nodex_lang_rust::lower_source;

#[test]
fn explains_vis_fragment_even_when_repeated_without_treating_transcriber_as_matcher() {
    let source = "macro_rules! m { ($v:vis struct $n:ident;) => { $v struct $n; }; ($($visibility:vis struct $name:ident;)*) => { $($visibility struct $name;)* }; } macro_rules! no { ($x:tt) => { $x:vis }; }";
    let file = lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|symbol| symbol.name == "m!").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro visibility fragment").collect();
    assert_eq!(items.len(), 2, "{:?}", symbol.rust_contract);
    assert!(items.iter().any(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize] == "$v:vis"));
    assert!(items.iter().any(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize] == "$visibility:vis"));
    assert!(items.iter().all(|item| item.text.contains("match no visibility tokens") && item.text.contains("does not by itself make an item public") && item.text.contains("must make progress")));
    let other = file.symbols.iter().find(|symbol| symbol.name == "no!").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Macro visibility fragment"));
}
