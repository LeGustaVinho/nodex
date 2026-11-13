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
        if branches[0].pin.is_none() && branches[0].output == "Use the value to continue evaluating the call"
            && branches[1].action == "Return from this closure"));
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
        if branches[0].pin.is_none() && branches[0].output == "Use the value to continue evaluating the call"
            && branches[1].pin.as_deref() == Some("error")));
}

#[test]
fn question_mark_success_finishes_the_surrounding_expression_before_advancing() {
    let source = r#"
        fn catalog(input: Option<Result<Vec<u8>, String>>) -> Result<Vec<u8>, String> {
            let result = (input.transpose()?).unwrap_or_default();
            Ok(result)
        }
        fn plain(input: Option<Result<Vec<u8>, String>>) -> Result<Option<Vec<u8>>, String> {
            let result = (input.transpose()?);
            Ok(result)
        }
        fn standalone(input: Option<Result<Vec<u8>, String>>) -> Result<(), String> {
            input.transpose()?;
            Ok(())
        }
        fn field(input: Result<(u8, u8), String>) -> Result<u8, String> {
            let result = input?.0;
            Ok(result)
        }
        fn cast(input: Result<i32, String>) -> Result<i64, String> {
            let result = input? as i64;
            Ok(result)
        }
        fn chain(input: Result<Option<u8>, String>) -> Result<u8, String> {
            let result = input?.ok_or(String::new())?;
            Ok(result)
        }
    "#;
    let ir = lower_source("src/lib.rs", source);
    for (name, output, pin) in [
        ("catalog", "Use the value to continue evaluating the call", None),
        ("plain", "Continue", Some("out")),
        ("standalone", "Continue", Some("out")),
        ("field", "Use the value to continue evaluating the field access", None),
        ("cast", "Use the value to continue evaluating this expression", None),
    ] {
        let graph = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        let (node, site) = question_marks(graph, source).next().unwrap();
        assert!(matches!(&site.content, ReadingContent::Paths { branches, .. }
            if branches[0].pin.as_deref() == pin && branches[0].output == output
                && branches[1].pin.as_deref() == Some("error")), "{name}: {site:?}");
        assert!(graph.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Exec
            && edge.from == node.id && edge.from_pin == "out"));
    }
    let graph = ir.flows.iter().find(|flow| flow.name == "chain").unwrap();
    let mut sites = question_marks(graph, source).map(|(_, site)| site).collect::<Vec<_>>();
    sites.sort_by_key(|site| site.span.start_byte);
    let [first, last] = sites.as_slice() else { panic!("{sites:?}") };
    assert!(matches!(&first.content, ReadingContent::Paths { branches, .. }
        if branches[0].pin.is_none() && branches[0].output == "Use the value to continue evaluating the call"));
    assert!(matches!(&last.content, ReadingContent::Paths { branches, .. }
        if branches[0].pin.as_deref() == Some("out")));
}

#[test]
fn question_mark_at_the_end_of_a_block_feeds_the_enclosing_call() {
    let source = r#"
        fn argument(input: Result<u8, String>) -> Result<u8, String> {
            let value = std::hint::black_box({ input? });
            Ok(value)
        }
        fn receiver(input: Result<Option<u8>, String>) -> Result<u8, String> {
            let value = ({ { input? } }).unwrap_or_default();
            Ok(value)
        }
        fn not_tail(input: Result<u8, String>) -> Result<u8, String> {
            let value = std::hint::black_box({ input?; 1 });
            Ok(value)
        }
        fn discarded_tail(input: Result<u8, String>) -> Result<(), String> {
            std::hint::black_box({ input?; });
            Ok(())
        }
        fn standalone(input: Result<u8, String>) -> Result<u8, String> {
            let value = { input? };
            Ok(value)
        }
        fn callback(input: Result<u8, String>) -> Result<u8, String> {
            let callback = std::hint::black_box(|| -> Result<u8, String> { Ok({ input? }) });
            callback()
        }
        async fn future_boundary(input: Result<u8, String>) -> Result<u8, String> {
            std::hint::black_box(async { Ok::<u8, String>(input?) }).await
        }
        async fn future_tail(input: Result<Result<u8, String>, String>) -> Result<u8, String> {
            std::hint::black_box(async { input? }).await
        }
    "#;
    let ir = lower_source("src/lib.rs", source);
    for (function, pin, resumed) in [
        ("argument", None, "Use the value to continue evaluating the call"),
        ("receiver", None, "Use the value to continue evaluating the call"),
        ("not_tail", Some("out"), "Continue"),
        ("discarded_tail", Some("out"), "Continue"),
        ("standalone", Some("out"), "Continue"),
        ("black_box callback", None, "Use the value to continue evaluating the call"),
    ] {
        let graph = ir.flows.iter().find(|flow| flow.name == function)
            .unwrap_or_else(|| panic!("Missing {function}: {:?}", ir.flows.iter().map(|flow| &flow.name).collect::<Vec<_>>()));
        let (node, question) = question_marks(graph, source).next().unwrap();
        assert!(matches!(&question.content, ReadingContent::Paths { branches, .. }
            if branches[0].pin.as_deref() == pin && branches[0].output == resumed
                && branches[1].pin.as_deref() == Some("error")), "{function}: {question:?}");
        if function != "black_box callback" {
            assert!(graph.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Exec
                && edge.from == node.id && edge.from_pin == "out"));
        } else {
            assert!(matches!(&question.content, ReadingContent::Paths { branches, .. }
                if branches[1].action == "Return from this callback"));
        }
    }
    let future = ir.flows.iter().find(|flow| flow.name == "future_boundary").unwrap();
    let (_, question) = question_marks(future, source).next().unwrap();
    assert!(matches!(&question.content, ReadingContent::Paths { branches, .. }
        if branches[1].action == "Finish this async block"));
    let future_tail = ir.flows.iter().find(|flow| flow.name == "future_tail").unwrap();
    let (_, question) = question_marks(future_tail, source).next().unwrap();
    assert!(matches!(&question.content, ReadingContent::Paths { branches, .. }
        if branches[0].pin.as_deref() == Some("out")
            && branches[0].output == "Continue"
            && branches[1].action == "Finish this async block"));
}
