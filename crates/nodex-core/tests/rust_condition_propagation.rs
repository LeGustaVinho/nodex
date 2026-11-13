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

#[test]
fn let_else_tests_the_pattern_after_successful_propagation() {
    let source = r#"
        fn visit_map(next_key: impl FnOnce() -> Result<Option<String>, &'static str>) -> Result<String, &'static str> {
            let Some(key) = next_key()? else { return Err("missing bits"); };
            Ok(key)
        }
        enum Value { Number(u32), Text }
        fn resistance(member: impl FnOnce() -> Result<Value, &'static str>) -> Result<u32, &'static str> {
            let Value::Number(amount) = member()? else { return Err("numeric damage pattern amount"); };
            Ok(amount)
        }
        fn alternate(input: Option<u32>, fallback: impl FnOnce() -> Option<u32>) -> Option<u32> {
            let Some(value) = input else { return Some(fallback()?); };
            Some(value)
        }
    "#;
    let ir = lower_source("src/lib.rs", source);
    for (name, fallback) in [("visit_map", "missing bits"), ("resistance", "numeric damage pattern amount")] {
        let graph = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        let (node, question) = question_marks(graph, source).next().unwrap();
        assert!(node.flavors.iter().any(|flavor| flavor == "let-else"));
        assert!(node.outputs.iter().any(|pin| pin.id == "error"));
        assert!(matches!(&question.content, ReadingContent::Paths { branches, .. }
            if branches[0].pin.is_none()
                && branches[0].output == "Continue evaluating the initializer before checking the let-else pattern"
                && branches[1].pin.as_deref() == Some("error") && branches[1].action == format!("Return from {name}")));

        let decision = graph.reading.iter().flatten().find(|site| site.node == node.id && site.path.is_none()
            && &source[site.span.start_byte as usize..site.span.end_byte as usize] == "else").unwrap();
        assert!(matches!(&decision.content, ReadingContent::Paths { branches, .. }
            if branches[0].pin.as_deref() == Some("out") && branches[1].pin.as_deref() == Some("else")));
        let edge = |pin| graph.edges.iter().find(|edge| edge.kind == FlowEdgeKind::Exec && edge.from == node.id && edge.from_pin == pin);
        let matched = graph.node(&edge("out").unwrap().to).unwrap();
        let mismatched = graph.node(&edge("else").unwrap().to).unwrap();
        assert!(matched.code.starts_with("Ok("));
        assert!(mismatched.code.contains(fallback));
        assert!(edge("error").is_none());
    }

    let alternate = ir.flows.iter().find(|flow| flow.name == "alternate").unwrap();
    let (node, question) = question_marks(alternate, source).next().unwrap();
    assert_eq!(node.concept, Concept::Return);
    assert!(matches!(&question.content, ReadingContent::Paths { branches, .. }
        if branches[0].pin.as_deref() == Some("out") && branches[1].pin.as_deref() == Some("error")));
}
