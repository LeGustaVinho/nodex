use nodex_ir::{Concept, FlowEdgeKind, FlowGraph, FlowNode, reading::{ReadingContent, ReadingSite}};
use nodex_lang_rust::lower_source;

fn loop_condition(flow: &FlowGraph) -> (&FlowNode, &ReadingSite) {
    let node = flow.nodes.iter().find(|node| node.concept == Concept::LoopWhile).unwrap();
    let site = flow.reading.iter().flatten().find(|site| site.node == node.id && matches!(&site.content,
        ReadingContent::Paths { timing: Some(timing), .. } if timing == "Before each loop iteration")).unwrap();
    assert!(site.path.is_none());
    (node, site)
}

fn snippet<'a>(source: &'a str, site: &ReadingSite) -> &'a str {
    &source[site.span.start_byte as usize..site.span.end_byte as usize]
}

#[test]
fn while_let_repeats_the_pattern_check_and_explains_unmapped_exit() {
    let source = r#"
        fn room_path(queue: &mut std::collections::VecDeque<usize>, to: usize) -> bool {
            while let Some(current) = queue.pop_front() {
                if current == to { return true; }
            }
            false
        }
        async fn durable(mut receive: impl AsyncFnMut() -> Option<u32>) {
            while let Some(first) = receive().await {
                std::hint::black_box(first);
            }
        }
        fn always(next: impl Fn() -> u8) {
            while let _ = next() { break; }
        }
        #[allow(non_upper_case_globals)]
        const sentinel: u8 = 0;
        fn compare(next: impl Fn() -> u8) {
            while let sentinel = next() { break; }
        }
    "#;
    let ir = lower_source("src/lib.rs", source);
    let graph = |name| ir.flows.iter().find(|flow| flow.name == name).unwrap();
    for (name, value, mapped_out) in [("room_path", "queue.pop_front()", true), ("durable", "receive().await", false)] {
        let flow = graph(name);
        let (node, site) = loop_condition(flow);
        assert_eq!(snippet(source, site), "while");
        let ReadingContent::Paths { input, branches, deferred, .. } = &site.content else { unreachable!() };
        assert!(!deferred);
        assert!(input.starts_with(&format!("On each attempt, evaluate {value}")), "{input}");
        assert_eq!(branches.iter().map(|branch| branch.pin.as_deref()).collect::<Vec<_>>(), [Some("body"), Some("out")]);
        assert_eq!(branches[1].action, "End the loop without running this iteration.");
        assert!(flow.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Exec && edge.from == node.id && edge.from_pin == "body"));
        assert_eq!(flow.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Exec && edge.from == node.id && edge.from_pin == "out"), mapped_out);
        assert_eq!(input.contains("The task can suspend while waiting"), name == "durable");
    }

    let (_, site) = loop_condition(graph("always"));
    assert!(matches!(&site.content, ReadingContent::Paths { input, branches, .. }
        if input.contains("accepts every value") && branches.len() == 1 && branches[0].pin.as_deref() == Some("body")));
    let (_, site) = loop_condition(graph("compare"));
    assert!(matches!(&site.content, ReadingContent::Paths { branches, .. } if branches.len() == 2));
}

#[test]
fn loop_chain_short_circuits_and_question_mark_success_keeps_checking() {
    let source = r#"
        fn catalog(mut next: impl FnMut() -> Result<Option<u8>, String>) -> Result<u8, String> {
            let mut total = 0;
            while let Some(value) = next()? && value != 0 {
                total += value;
            }
            Ok(total)
        }
        fn nested(result: Result<Option<u8>, String>) -> Result<(), String> {
            while let Some(value) = { result.clone()? } && value > 10 {
                break;
            }
            Ok(())
        }
        fn nested_if(mut next: impl FnMut() -> Option<u8>) {
            while let Some(value) = { if let Some(inner) = next() && inner > 1 { Some(inner) } else { Some(1) } } {
                std::hint::black_box(value);
                break;
            }
        }
    "#;
    let ir = lower_source("src/lib.rs", source);
    for name in ["catalog", "nested"] {
        let graph = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        let (node, site) = loop_condition(graph);
        assert!(matches!(&site.content, ReadingContent::Paths { branches, .. }
            if branches[0].pin.as_deref() == Some("body") && branches[1].pin.as_deref() == Some("out")
                && branches[1].action.contains("Skip the remaining checks")));
        let sites = graph.reading.as_ref().unwrap();
        let order = sites.iter().find(|site| site.node == node.id && snippet(source, site) == "&&").unwrap();
        assert!(matches!(&order.content, ReadingContent::Steps { steps, .. }
            if steps[0].timing == "First on each attempt" && steps[1].timing == "If all earlier checks pass"));
        let question = sites.iter().find(|site| site.node == node.id && snippet(source, site) == "?").unwrap();
        assert!(matches!(&question.content, ReadingContent::Paths { branches, .. }
            if branches[0].pin.is_none() && branches[0].output == "Continue checking the loop condition"
                && branches[1].pin.as_deref() == Some("error") && branches[1].action.starts_with("Return from")));
        assert!(node.outputs.iter().any(|pin| pin.id == "error") || name == "nested");
    }
    let graph = ir.flows.iter().find(|flow| flow.name == "nested_if").unwrap();
    let (node, _) = loop_condition(graph);
    let sites = graph.reading.as_ref().unwrap();
    assert!(sites.iter().any(|site| site.node == node.id && snippet(source, site) == "&&" && matches!(&site.content,
        ReadingContent::Steps { steps, .. } if steps[0].timing == "First" && !steps.last().unwrap().action.contains("leave the loop"))));
    assert!(sites.iter().any(|site| site.node == node.id && matches!(&site.content,
        ReadingContent::Pattern { route: Some(pin), parts, .. } if pin == "true" && parts.iter().any(|part| part.binding.as_deref() == Some("inner")))));
}
