use nodex_lang_rust::lower_source;

#[test]
fn explains_gdb_visualizer_and_preserves_its_path_on_the_correct_module() {
    let source = "#![debugger_visualizer(gdb_script_file = \"a.py\")]\nmod inner { #![debugger_visualizer(gdb_script_file = \"dir/b.py\")] } mod other { #![debugger_visualizer(natvis_file = \"a.natvis\")] }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none()).unwrap();
    let detail = root.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "GDB debugger visualizer").unwrap();
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "#![debugger_visualizer(gdb_script_file = \"a.py\")]");
    assert!(detail.text.contains("\"a.py\"") && detail.text.contains("relative to the source file") && detail.text.contains("auto-load permission") && detail.text.contains("does not prove"));
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    assert!(inner.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "GDB debugger visualizer" && item.text.contains("\"dir/b.py\"")));
    let other = ir.symbols.iter().find(|symbol| symbol.name == "other").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "GDB debugger visualizer"));
}
