use nodex_ir::{Access, Concept, Expr, FlowEdgeKind, FlowGraph, reading::ReadingContent};
use nodex_lang_rust::lower_source;

fn lambda(expression: &Expr) -> Option<&Expr> {
    if expression.concept == Concept::Lambda {
        Some(expression)
    } else {
        expression.children.iter().find_map(lambda)
    }
}

fn constructs_range(graph: &FlowGraph) -> bool {
    graph.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Range stops before the end"))
}

#[test]
fn opens_the_retirement_callback_and_keeps_its_receiver_capture_on_creation() {
    let source = r#"
        use std::sync::{Arc, mpsc};

        fn retirement() -> mpsc::Sender<Arc<u64>> {
            let (sender, receiver) = mpsc::channel::<Arc<u64>>();
            std::thread::Builder::new()
                .name("publication-retirement".into())
                .spawn(move || while receiver.recv().is_ok() {})
                .expect("the retirement thread starts");
            sender
        }
    "#;
    let ir = lower_source("src/battle.rs", source);
    let retirement = ir.flows.iter().find(|flow| flow.name == "retirement").unwrap();
    let callback = ir.flows.iter().find(|flow| flow.name == "spawn callback").unwrap();
    let creation = retirement.nodes.iter().find(|node| node.code.contains(".spawn(")).unwrap();
    let function = lambda(creation.expr.as_ref().unwrap()).unwrap();
    assert_eq!(function.symbol.as_deref(), Some(callback.symbol.as_str()));
    assert!(function.children.is_empty());
    assert!(function.flavors.iter().any(|flavor| flavor == "move"));
    assert!(creation.calls.iter().any(|call| call.symbol.as_deref() == Some(callback.symbol.as_str())));
    assert!(retirement.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Data && edge.to == creation.id
        && edge.to_pin == "use:receiver" && matches!(edge.access, Some(Access::ByValue | Access::Move))));
    let entry = callback.nodes.first().unwrap();
    assert_eq!(entry.label, "when spawn calls this function");
    assert!(!entry.outputs.iter().any(|pin| pin.label == "receiver"));
    let predicate = callback.nodes.iter().find(|node| node.concept == Concept::LoopWhile).unwrap();
    assert!(predicate.code.contains("receiver.recv().is_ok()"));
    assert!(callback.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Exec && edge.from == entry.id && edge.to == predicate.id));
    assert!(!callback.nodes.iter().any(|node| node.concept == Concept::Return));
}

#[test]
fn direct_loops_show_results_in_the_child_and_keep_simple_closures_inline() {
    let source = r#"
        fn invoke<R>(callback: impl FnOnce() -> R) -> R { callback() }

        fn controls() {
            let bound = 7;
            let make = move || (loop { break bound; });
            let deferred = async || loop { break 8; };
            let from_future = || async { loop { break 9; } };
            let simple = || 10;
            let wrapped = || Some(loop { break 11; });
            let empty = || { /* empty */ };
            let inline_map = [1, 2].map(|x| x + 1);
            let iterations = invoke((|| for value in 0..3 { let _ = value; }));
        }
    "#;
    let ir = lower_source("src/lib.rs", source);
    let flow = |name| ir.flows.iter().find(|flow| flow.name == name);
    let controls = flow("controls").unwrap();

    for (name, operand) in [("make closure", "break bound"), ("deferred closure", "break 8")] {
        let child = flow(name).unwrap();
        assert!(child.nodes.iter().any(|node| node.concept == Concept::LoopInfinite));
        assert!(child.nodes.iter().any(|node| node.concept == Concept::Break && node.code.starts_with(operand)
            && node.flavors.iter().any(|flavor| flavor == "break-value-return")));
    }
    let deferred = flow("deferred closure").unwrap();
    assert_eq!(deferred.nodes[0].label, "when the future from this async closure is polled");
    assert!(deferred.nodes[0].flavors.iter().any(|flavor| flavor == "async"));
    assert!(controls.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, text, .. } if label == "Create function deferred()" && text.contains("creates a future"))));

    let callback = flow("invoke callback").unwrap();
    assert!(callback.nodes.iter().any(|node| node.concept == Concept::LoopForEach));
    assert!(constructs_range(callback));
    assert!(!constructs_range(controls));
    for name in ["from_future closure", "simple closure", "wrapped closure", "empty closure", "map callback"] {
        assert!(flow(name).is_none(), "Unexpected Blueprint for {name}");
    }
    let inline = controls.nodes.iter().find(|node| node.code.starts_with("let simple")).unwrap();
    let simple = lambda(inline.expr.as_ref().unwrap()).unwrap();
    assert!(simple.symbol.is_none());
    assert_eq!(simple.children.len(), 1);
}
