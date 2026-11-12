use std::{path::Path, sync::Arc};

use nodex_core::Project;
use nodex_ir::{
    Expr, FlowGraph, Language, SyntaxSite, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_rust::RustFrontend;

fn cast_details(graph: &FlowGraph) -> impl Iterator<Item = (&ReadingSite, &str, &str)> {
    graph
        .reading
        .iter()
        .flatten()
        .filter_map(|site| match &site.content {
            ReadingContent::Detail { label, text, .. } if label.starts_with("Convert ") => {
                Some((site, label.as_str(), text.as_str()))
            }
            _ => None,
        })
}

fn syntax_sites<'a>(expr: &'a Expr, sites: &mut Vec<&'a SyntaxSite>) {
    sites.extend(expr.syntax_sites.iter().flatten());
    for child in &expr.children {
        syntax_sites(child, sites);
    }
}

#[test]
fn rust_casts_explain_only_the_numeric_rules_with_known_types() {
    let source = r#"
        use std::mem::size_of as size;
        enum Tag { Idle }
        type Count = u16;
        fn casts(p: *const i32, maybe: u16) {
            let truncated = -3.75_f64 as i16;
            let narrowed = 511_u16 as u8;
            let reinterpreted = 255_u8 as i8;
            let signed = -1_i8 as u16;
            let unsigned = 255_u8 as i16;
            let nested = 511_u16 as u8 as u32;
            let exact = 17_u16 as f32;
            let rounded = 17_i64 as f32;
            let float = 1.25_f64 as f32;
            let widened = 1.25_f32 as f64;
            let arch = 1_u64 as usize;
            let arch_input = 1_usize as u64;
            let pointer = p as *const u8;
            let address = p as u64;
            let tag = Tag::Idle as u8;
            let alias = 1_u64 as Count;
            let unknown = maybe as u8;
        }
    "#;
    let ir = nodex_lang_rust::lower_source("casts.rs", source);
    assert!(ir.diagnostics.is_empty(), "{:?}", ir.diagnostics);
    let graph = ir.flows.iter().find(|flow| flow.name == "casts").unwrap();
    let details: Vec<_> = cast_details(graph).collect();
    assert_eq!(details.len(), 11, "{details:?}");
    for (site, _, _) in &details {
        assert_eq!(site.basis, ValueBasis::Syntax);
        assert_eq!(
            &source[site.span.start_byte as usize..site.span.end_byte as usize],
            "as"
        );
    }
    for (label, text) in [
        ("Convert f64 to i16", "toward zero"),
        ("Convert f64 to i16", "-32768 through 32767"),
        ("Convert f64 to i16", "NaN becomes 0"),
        ("Convert u16 to u8", "lowest 8 bits"),
        ("Convert u8 to i8", "same 8 bits"),
        (
            "Convert i8 to u16",
            "negative input becomes a large positive number",
        ),
        ("Convert u8 to i16", "high bits with zeroes"),
        ("Convert u8 to u32", "reach 32 bits"),
        ("Convert u16 to f32", "fits exactly"),
        ("Convert i64 to f32", "even last bit"),
        ("Convert f64 to f32", "infinity with the same sign"),
        ("Convert f32 to f64", "every finite value exactly"),
    ] {
        assert!(
            details
                .iter()
                .any(|(_, found, body)| *found == label && body.contains(text)),
            "{label} · {text}"
        );
    }
    let mut badges = Vec::new();
    for node in &graph.nodes {
        badges.extend(node.syntax_sites.iter().flatten());
        if let Some(expr) = &node.expr {
            syntax_sites(expr, &mut badges);
        }
    }
    let cast_tokens: Vec<_> = badges
        .into_iter()
        .filter(|site| site.key == "type-cast")
        .map(|site| site.span.start_byte as usize)
        .collect();
    let expected: Vec<_> = source
        .match_indices(" as ")
        .map(|(at, _)| at + 1)
        .skip(1)
        .collect();
    assert_eq!(cast_tokens.len(), expected.len());
    assert!(expected.iter().all(|offset| cast_tokens.contains(offset)));
}

#[test]
fn rust_casts_keep_generic_syntax_when_a_primitive_name_can_be_shadowed() {
    let source = r#"
        pub fn ordinary() -> u8 { 257_u16 as u8 }
        mod local {
            type u8 = *const i32;
            pub fn target() -> u8 { 1_u64 as u8 }
        }
        mod raw {
            type r#u8 = *const i32;
            pub fn target() -> u8 { 1_u64 as u8 }
        }
        mod imported {
            type Address = *const i32;
            use self::Address as u8;
            pub fn target() -> u8 { 1_u64 as u8 }
        }
        mod glob_source { pub type u8 = *const i32; }
        mod glob {
            use super::glob_source::*;
            pub fn target() -> u8 { 1_u64 as u8 }
        }
        mod generated {
            macro_rules! alias { () => { type u8 = *const i32; }; }
            alias!();
            pub fn target() -> u8 { 1_u64 as u8 }
        }
    "#;
    let ir = nodex_lang_rust::lower_source("shadows.rs", source);
    assert!(ir.diagnostics.is_empty(), "{:?}", ir.diagnostics);
    assert_eq!(
        ir.flows.iter().filter(|flow| flow.name == "target").count(),
        5
    );
    for flow in &ir.flows {
        assert_eq!(
            cast_details(flow).count(),
            usize::from(flow.name == "ordinary"),
            "{}",
            flow.symbol
        );
        let mut sites = Vec::new();
        for node in &flow.nodes {
            sites.extend(node.syntax_sites.iter().flatten());
            if let Some(expr) = &node.expr {
                syntax_sites(expr, &mut sites);
            }
        }
        assert!(
            sites.iter().any(|site| site.key == "type-cast"),
            "{}",
            flow.symbol
        );
    }
}

#[test]
#[ignore = "needs rust-analyzer"]
fn rust_heading_cast_uses_the_resolved_return_type_from_another_file() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/language-details");
    let mut project = Project::open(&root, vec![Arc::new(RustFrontend)]).unwrap();
    let names = [
        "reading_heading_cast",
        "reading_field_cast",
        "reading_alias_target_cast",
        "reading_alias_source_cast",
        "reading_nominal_source_cast",
        "reading_inferred_nominal_source_cast",
        "reading_qualified_primitive_source_cast",
        "reading_inferred_primitive_source_cast",
    ];
    let structure = project.structure();
    let [
        symbol,
        field,
        alias_target,
        alias_source,
        nominal_source,
        inferred_nominal,
        qualified,
        inferred_primitive,
    ] = names.map(|name| {
        structure
            .symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap()
            .id
            .clone()
    });
    let before = project.flow(&symbol).unwrap();
    assert!(cast_details(&before).next().is_none());
    let mut provider = RustFrontend.semantic(project.root()).unwrap().unwrap();
    project.apply_semantic(Language::Rust, provider.analyze().unwrap());
    let after = project.flow(&symbol).unwrap();
    let details: Vec<_> = cast_details(&after).collect();
    let [(site, label, text)] = details.as_slice() else {
        panic!("{details:?}")
    };
    assert_eq!(*label, "Convert f64 to u16");
    assert_eq!(site.basis, ValueBasis::TypeAnalysis);
    assert!(text.contains("toward zero"));
    assert!(text.contains("0 through 65535"));
    assert!(text.contains("NaN becomes 0"));
    let source = include_str!("../../../fixtures/rosetta/language-details/src/rust_reading.rs");
    assert_eq!(
        &source[site.span.start_byte as usize..site.span.end_byte as usize],
        "as"
    );
    let field = project.flow(&field).unwrap();
    let (site, label, _) = cast_details(&field).next().unwrap();
    assert_eq!(label, "Convert f64 to i64");
    assert_eq!(site.basis, ValueBasis::TypeAnalysis);
    for id in [alias_target, alias_source, nominal_source, inferred_nominal] {
        let graph = project.flow(&id).unwrap();
        assert!(cast_details(&graph).next().is_none(), "{:?}", graph.reading);
    }
    for id in [qualified, inferred_primitive] {
        let graph = project.flow(&id).unwrap();
        let (site, label, _) = cast_details(&graph)
            .next()
            .unwrap_or_else(|| panic!("{:?}", graph.reading));
        assert_eq!(label, "Convert u8 to u64");
        assert_eq!(site.basis, ValueBasis::TypeAnalysis);
    }
}
