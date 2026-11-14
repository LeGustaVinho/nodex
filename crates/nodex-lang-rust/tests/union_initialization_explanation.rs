use nodex_ir::reading::ReadingContent;

#[test]
fn union_initializer_is_explained_on_the_flow_without_assuming_resolution() {
    let source = "union Bits { i: u32, f: f32 } fn run() { let value = Bits { i: 1 }; }";
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "run").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(
        &site.content,
        ReadingContent::Detail { label, text, .. }
            if label == "Initialize a union field"
                && text.contains("If Bits here resolves")
                && text.contains("exactly one field")
                && text.contains("unsafe")
    )));
}
