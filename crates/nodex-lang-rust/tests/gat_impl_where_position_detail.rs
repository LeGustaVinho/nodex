use nodex_lang_rust::lower_source;

#[test]
fn explains_generic_associated_type_where_after_the_assigned_type() {
    let source = "trait T { type Item<U> where U: Clone; } struct S; impl T for S { type Item<U> = Vec<U> where U: Clone; } struct X; impl T for X { type Item<U> where U: Clone = Vec<U>; } type Alias<U> = Vec<U> where U: Clone;";
    let ir = lower_source("src/lib.rs", source);
    let items: Vec<_> = ir.symbols.iter().filter(|symbol| symbol.name == "Item").collect();
    let details: Vec<_> = items.iter().flat_map(|symbol| symbol.rust_contract.as_ref().into_iter().flatten()).filter(|item| item.group == "Generic associated type trailing where").collect();
    assert_eq!(details.len(), 1, "{items:?}");
    let detail = details[0];
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "where U: Clone");
    assert!(detail.text.contains("Item<U>") && detail.text.contains("after the assigned type") && detail.text.contains("before `=` is deprecated") && detail.text.contains("compatible with the trait") && detail.text.contains("If Clone resolves") && detail.text.contains("If Vec resolves"));
}
