use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_target_sized_integers_in_signatures_and_local_bindings() {
    let source = "fn f(n: Vec<usize>) -> isize { let x: Option<usize> = None; 0 as isize }";
    let ir = lower_source("src/lib.rs", source);
    let function = ir.symbols.iter().find(|s| s.name == "f").unwrap();
    let details: Vec<_> = function.rust_contract.iter().flatten().filter(|c| c.group == "Pointer-sized integer").collect();
    assert_eq!(details.len(), 2);
    assert!(details.iter().any(|c| c.text.contains("0 through 2^N - 1")));
    assert!(details.iter().any(|c| c.text.contains("-2^(N-1) through 2^(N-1) - 1")));
    assert!(details.iter().all(|c| c.text.contains("target platform")));
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    assert!(flow.reading.iter().flatten().any(|r| matches!(&r.content, ReadingContent::Detail { label, text, .. } if label == "Pointer-sized integer" && text.contains("usize"))));
}
