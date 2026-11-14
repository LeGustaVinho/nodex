use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn signed_local_annotations_are_explained_in_flow_without_erasing_type_annotation() {
    let text = "fn f() { let x: i8 = 1; let y: Option<i16> = None; let z: (i32, i64) = (0, 0); }";
    let ir = lower_source("src/lib.rs", text);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let readings = flow.reading.as_ref().unwrap();
    for ty in ["i8", "i16", "i32", "i64"] {
        assert!(readings.iter().any(|site| matches!(&site.content, ReadingContent::Detail { label, text, .. } if label == "Signed integer type" && text.contains(&format!("{ty} is a signed")))), "{ty}: {readings:?}");
    }
    assert!(readings.iter().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Explicit variable type")));
}
