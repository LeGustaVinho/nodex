use nodex_ir::TypeRelation;
use nodex_lang_rust::lower_source;

#[test]
fn explains_negative_impl_and_does_not_report_a_positive_relation() {
    let source = "trait Marker {} struct S; struct P; impl ! /* comment */ Marker for S {} impl Marker for P {}";
    let ir = lower_source("src/lib.rs", source);
    let target = ir.symbols.iter().find(|symbol| symbol.name == "S").unwrap();
    let detail = target.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Negative trait implementation").unwrap();
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "!");
    assert!(detail.text.contains("does not implement") && detail.text.contains("unstable") && detail.text.contains("feature configuration") && detail.text.contains("auto trait") && detail.text.contains("does not establish"));
    assert!(!target.flavors.iter().any(|flavor| flavor == "impl:Marker"));
    assert!(!ir.type_refs.iter().any(|reference| reference.from == target.id && reference.name == "Marker" && reference.relation == TypeRelation::Base));
    let positive = ir.symbols.iter().find(|symbol| symbol.name == "P").unwrap();
    assert!(positive.flavors.iter().any(|flavor| flavor == "impl:Marker"));
    assert!(!positive.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Negative trait implementation"));
}
