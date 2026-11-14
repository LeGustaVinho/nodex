use nodex_lang_rust::lower_source;

#[test]
fn infers_raw_pointer_types_without_confusing_reference_types() {
    let source = "fn f(mut x: i32) { let raw_const = &raw const x; let raw_mut = &raw mut x; let shared = &x; let exclusive = &mut x; }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    for (name, expected) in [
        ("raw_const", "*const i32"),
        ("raw_mut", "*mut i32"),
        ("shared", "&i32"),
        ("exclusive", "&mut i32"),
    ] {
        let pin = flow.nodes.iter().flat_map(|node| &node.outputs)
            .find(|pin| pin.id == format!("var:{name}"))
            .unwrap_or_else(|| panic!("no binding {name}"));
        assert_eq!(pin.ty.as_deref(), Some(expected), "{name}: {pin:?}");
    }
}
