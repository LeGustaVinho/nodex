use nodex_lang_rust::lower_source;

fn shebang(source: &str) -> Option<(String, String)> {
    let ir = lower_source("src/lib.rs", source);
    let module = ir.symbols.iter().find(|symbol| symbol.flavors.iter().any(|flavor| flavor == "crate-root")).unwrap();
    module.rust_contract.as_ref().into_iter().flatten().find(|item| item.group == "Shebang")
        .map(|item| (source[item.span.start_byte as usize..item.span.end_byte as usize].to_owned(), item.text.clone()))
}

#[test]
fn shebang_is_explained_on_the_file_module_without_confusing_inner_attributes() {
    let (span, text) = shebang("#!/usr/bin/env rustx\nfn main() {}\n").unwrap();
    assert_eq!(span, "#!/usr/bin/env rustx");
    assert!(text.contains("interpreter") && text.contains("Rust removes") && text.contains("does not itself run or compile") && text.contains("#![...]"));
    let (span, _) = shebang("\u{feff}#!/usr/bin/env rustx\r\nfn main() {}\n").unwrap();
    assert_eq!(span, "#!/usr/bin/env rustx");
    assert!(shebang("#![allow(unused)]\nfn main() {}\n").is_none());
    assert!(shebang("// comment\n#!/usr/bin/env rustx\nfn main() {}\n").is_none());
}
