use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn explains_typed_size_of_only_for_supported_paths_and_no_arguments() {
    let source = "fn f<T>() { std::mem::size_of::<T>(); core::mem::size_of::<()>(); other::size_of::<T>(); std::mem::size_of::<T>(x); }";
    let file = lower_source("src/lib.rs", source);
    let flow = file.flows.iter().find(|flow| flow.name == "f").unwrap();
    let details: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Size of a type")).collect();
    assert_eq!(details.len(), 2, "{:?}", flow.reading);
    for site in details {
        assert!(matches!(&site.content, ReadingContent::Detail { text, .. } if text.contains("If ") && text.contains("count of bytes") && text.contains("padding") && text.contains("zero-sized") && text.contains("target") && text.contains("name resolution")));
    }
}
