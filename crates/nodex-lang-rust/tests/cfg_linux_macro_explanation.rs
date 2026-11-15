use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn cfg_linux_is_explained_conditionally_and_not_as_runtime_os_detection() {
    let source = "fn f() { let linux = cfg!(target_os = \"linux\"); let windows = cfg!(target_os = \"windows\"); let other = other!(target_os = \"linux\"); }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Check the compilation target")).collect();
    assert_eq!(sites.len(), 1, "{:?}", flow.reading);
    let site = sites[0];
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "target_os");
    assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("If `cfg!` resolves")
        && text.contains("compilation target") && text.contains("not a runtime")
        && text.contains("does not itself remove surrounding branches") && text.contains("true") && text.contains("false")));
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Macro invocation")));
}
