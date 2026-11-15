use nodex_ir::reading::ReadingContent;
use nodex_lang_rust::lower_source;

#[test]
fn underscore_prefixed_let_name_is_binding_not_wildcard() {
    let source = "fn f() { let _unused = 1; let _used = make(); use_it(_used); let _ = make(); let plain = 1; let text = \"_unused\"; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|f| f.name == "f").unwrap();
    let items: Vec<_> = flow.reading.iter().flatten().filter_map(|site| match &site.content { ReadingContent::Detail { label, text, .. } if label == "Underscore-prefixed local binding" => Some((site, text)), _ => None }).collect();
    assert_eq!(items.len(), 2, "{:?}", flow.reading);
    for (site, text) in items {
        let name = &source[site.span.start_byte as usize..site.span.end_byte as usize];
        assert!(["_unused", "_used"].contains(&name));
        for part in ["can still be used", "unused_variables", "initializer's effects", "wildcard", "drop point", "moved or copied"] { assert!(text.contains(part), "{text}"); }
    }
}
