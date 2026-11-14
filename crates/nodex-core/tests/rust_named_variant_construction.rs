use nodex_ir::reading::ReadingContent;

#[test]
fn explains_named_variant_construction_in_the_flow_without_assuming_resolution() {
    let source = "enum Shape { Circle { radius: f32 } } fn make() { let s = Shape::Circle { radius: 2.0 }; }";
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "make").unwrap();
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, text, .. }
        if label == "Possible named enum variant" && text.contains("active variant is Circle") && text.contains("depends on name resolution")
    )));
}
