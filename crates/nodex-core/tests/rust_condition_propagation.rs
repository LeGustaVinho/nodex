use nodex_ir::{Concept, FlowEdgeKind, FlowGraph, FlowNode, reading::{ReadingContent, ReadingSite}};
use nodex_lang_rust::lower_source;

fn question_marks<'a>(graph: &'a FlowGraph, source: &'a str) -> impl Iterator<Item = (&'a FlowNode, &'a ReadingSite)> {
    graph.reading.iter().flatten().filter_map(move |site| {
        let text = &source[site.span.start_byte as usize..site.span.end_byte as usize];
        (text == "?").then(|| (graph.node(&site.node).unwrap(), site))
    })
}

#[test]
fn successful_propagation_in_a_decision_keeps_evaluating_that_decision() {
    let source = r#"
        fn admit_hull(projected: bool, assist: impl FnOnce() -> Result<bool, String>) -> Result<u8, String> {
            if !projected || !assist()? { std::hint::black_box(1); }
            Ok(2)
        }
        fn amount(member: impl FnOnce() -> Result<Option<u8>, String>) -> Result<u8, String> {
            let count = match member()? { Some(n) => n, None => 0 };
            Ok(count)
        }
        fn scan(member: impl FnOnce() -> Result<Option<u8>, String>) -> Result<u8, String> {
            match member()? {
                Some(n) => { std::hint::black_box(n); }
                None => { std::hint::black_box(0); }
            }
            Ok(3)
        }
        fn items(read: impl FnOnce() -> Result<Vec<u8>, String>) -> Result<u8, String> {
            for value in read()? { std::hint::black_box(value); }
            Ok(4)
        }
        fn guard(test: impl Fn() -> Result<bool, String>) -> Result<u8, String> {
            let count = match 1 { n if test()? => n, _ => 0 };
            Ok(count)
        }
        fn nested(value: Result<bool, String>) -> Result<u8, String> {
            let count = if { let flag = value?; flag } { 5 } else { 6 };
            Ok(count)
        }
        fn nested_return(flag: bool, value: Result<u8, String>) -> Result<u8, String> {
            let count = match if flag { return Ok(value?); } else { Some(7) } { Some(n) => n, None => 0 };
            Ok(count)
        }
        fn plain(value: Result<u8, String>) -> Result<u8, String> {
            let count = value?;
            Ok(count)
        }
        fn nested_callback(value: Result<u8, String>) -> Result<u8, String> {
            let callback = || -> Result<u8, String> { Ok(value?) };
            if callback().is_ok() { std::hint::black_box(7); }
            Ok(8)
        }
    "#;
    let ir = lower_source("src/lib.rs", source);
    for (name, concept, resumed) in [
        ("admit_hull", Concept::Branch, "Continue checking the if condition"),
        ("amount", Concept::Var, "Continue evaluating the value to match"),
        ("scan", Concept::Match, "Continue evaluating the value to match"),
        ("items", Concept::LoopForEach, "Continue evaluating what the loop iterates over"),
        ("guard", Concept::Var, "Continue checking the match guard"),
        ("nested", Concept::Var, "Continue checking the if condition"),
        ("nested_return", Concept::Var, "Continue with the value"),
    ] {
        let graph = ir.flows.iter().find(|graph| graph.name == name).unwrap();
        let mut sites = question_marks(graph, source);
        let (node, site) = sites.next().unwrap_or_else(|| panic!("Missing ? in {name}"));
        assert!(sites.next().is_none());
        assert_eq!(node.concept, concept, "{name}");
        assert!(matches!(&site.content, ReadingContent::Paths { branches, .. }
            if branches[0].pin.is_none() && branches[0].output == resumed
                && branches[1].pin.as_deref() == Some("error") && branches[1].action == format!("Return from {name}")));
        assert!(graph.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Exec && edge.from == node.id && edge.from_pin == "out"));
    }

    let plain = ir.flows.iter().find(|graph| graph.name == "plain").unwrap();
    let (node, site) = question_marks(plain, source).next().unwrap();
    assert!(node.outputs.iter().any(|pin| pin.id == "error"));
    assert!(matches!(&site.content, ReadingContent::Paths { branches, .. }
        if branches[0].pin.as_deref() == Some("out") && branches[1].pin.as_deref() == Some("error")));

    let callback = ir.flows.iter().find(|graph| graph.name == "callback closure").unwrap();
    let (_, site) = question_marks(callback, source).next().unwrap();
    assert!(matches!(&site.content, ReadingContent::Paths { branches, .. }
        if branches[0].pin.as_deref() == Some("out") && branches[1].action == "Return from this closure"));
}
