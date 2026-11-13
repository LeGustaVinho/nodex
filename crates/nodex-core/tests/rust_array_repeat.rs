use nodex_ir::{Expr, ValueBasis, reading::ReadingContent};

#[test]
fn rust_repeat_uses_the_written_separator_and_distinguishes_value_evaluation() {
    let source = r#"
        const BUFFS: [u8; 2] = [0, 1];
        const EMPTY: Vec<u8> = Vec::new();

        fn ordinary<T: Copy + Default, const N: usize>() -> [T; N] {
            [T::default() /* value */; N]
        }

        fn repeat() {
            let _ = [0.0; BUFFS.len()];
            let _ = [String::new(); 0];
            let _ = [(/* grouping */ const { Vec::<u8>::new() }); 2];
            let _ = [const { Vec::<u8>::new() }; 0];
            let _ = [(EMPTY); 2];
            let _ = [";", "ordinary list"];
            let _ = [{ let n = 3; n }, 4];
            let _: [u8; 3] = [0; _];
            let _: [u8; 3] = [0; ((_ ))];
            let local = 7;
            let _ = [local; 2];
            let _ = vec![0; 3];
        }
    "#;
    let ir = nodex_lang_rust::lower_source("repeat.rs", source);
    assert!(ir.diagnostics.is_empty(), "{:?}", ir.diagnostics);
    let all = ir
        .flows
        .iter()
        .flat_map(|flow| flow.reading.iter().flatten());
    let mut cards = all
        .filter_map(|site| match &site.content {
            ReadingContent::Detail { label, text, .. }
                if label == "Build an array by repetition" =>
            {
                assert_eq!(site.basis, ValueBasis::Syntax);
                assert!(
                    site.path.is_some(),
                    "array reading has no expression path: {site:?}"
                );
                let span = &site.span;
                assert_eq!(
                    &source[span.start_byte as usize..span.end_byte as usize],
                    ";"
                );
                Some((span.start_byte, text.as_str()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    cards.sort_by_key(|card| card.0);
    let expected = [
        (
            "/* value */; N]",
            "N elements",
            "once, even for an empty array",
        ),
        (
            "; BUFFS.len()]",
            "BUFFS.len() elements",
            "once, even for an empty array",
        ),
        (
            "String::new(); 0]",
            "0 elements",
            "once, even for an empty array",
        ),
        ("}); 2]", "2 elements", "const block once for each element"),
        (
            "Vec::<u8>::new() }; 0]",
            "0 elements",
            "A length of zero skips it",
        ),
        (
            "(EMPTY); 2]",
            "2 elements",
            "A named constant or a const block",
        ),
        (
            "; _]",
            "infers the array length from context",
            "once, even for an empty array",
        ),
        (
            "; ((_ ))]",
            "infers the array length from context",
            "once, even for an empty array",
        ),
        ("[local; 2]", "2 elements", "An ordinary value expression"),
    ];
    assert_eq!(cards.len(), expected.len(), "{cards:?}");
    for ((at, text), (snippet, length, behavior)) in cards.iter().zip(expected) {
        let expected_offset = source.find(snippet).unwrap() + snippet.find(';').unwrap();
        assert_eq!(*at as usize, expected_offset);
        assert!(text.contains(length), "{text}");
        assert!(text.contains(behavior), "{text}");
    }

    fn inspect<'a>(expr: &'a Expr, arrays: &mut Vec<&'a Expr>) {
        if expr.label == "list" || expr.label == "repeated items" {
            arrays.push(expr);
        }
        for child in &expr.children {
            inspect(child, arrays);
        }
    }
    let mut arrays = Vec::new();
    let mut badge_offsets = Vec::new();
    for node in ir.flows.iter().flat_map(|flow| &flow.nodes) {
        badge_offsets.extend(
            node.syntax_sites
                .iter()
                .flatten()
                .filter(|site| site.key == "array-repeat")
                .map(|site| site.span.start_byte),
        );
        if let Some(expr) = &node.expr {
            inspect(expr, &mut arrays);
        }
    }
    for array in &arrays {
        badge_offsets.extend(
            array
                .syntax_sites
                .iter()
                .flatten()
                .filter(|site| site.key == "array-repeat")
                .map(|site| site.span.start_byte),
        );
    }
    badge_offsets.sort_unstable();
    badge_offsets.dedup();
    assert_eq!(
        badge_offsets,
        cards.iter().map(|card| card.0).collect::<Vec<_>>()
    );

    for code in [r#"[";", "ordinary list"]"#, "[{ let n = 3; n }, 4]"] {
        let list = arrays
            .iter()
            .find(|array| {
                &source[array.span.start_byte as usize..array.span.end_byte as usize] == code
            })
            .unwrap();
        assert_eq!(list.label, "list");
    }
}
