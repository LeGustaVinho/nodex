use nodex_ir::{Concept, FlowEdgeKind, ValueBasis, reading::ReadingContent};
use nodex_lang_rust::lower_source;

#[test]
fn loop_header_explains_claim_retry_without_adding_a_retry_edge() {
    let source = r#"
enum ClaimError { Held, Other }

async fn claim(standby: bool) -> Result<u64, ClaimError> {
    let lease = 'claim: loop {
        match store_claim().await {
            Ok(lease) => break 'claim lease,
            Err(ClaimError::Held) if standby => { sleep().await; }
            Err(error) => return Err(error),
        }
    };
    log(lease);
    Ok(lease)
}

fn while_control() {
    while true { break; }
}
"#;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "claim").unwrap();
    let loop_node = flow.nodes.iter().find(|node| node.concept == Concept::LoopInfinite).unwrap();
    assert_eq!(loop_node.label, "repeat the loop body");
    assert!(loop_node.flavors.iter().any(|flavor| flavor == "rust-loop"));
    let site = flow.reading.iter().flatten().find(|site| site.node == loop_node.id && matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Repeat the loop body")).unwrap();
    assert_eq!(site.basis, ValueBasis::Syntax);
    assert_eq!(site.path, None);
    assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "loop");
    let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
    for expected in ["without checking a condition", "If the body reaches its end, start it again", "continue targeting this loop",
        "break targeting this loop", "supplies the result of the loop expression", "A return can leave the enclosing function, closure, or async block"] {
        assert!(text.contains(expected));
    }

    let decision = flow.nodes.iter().find(|node| node.concept == Concept::Match).unwrap();
    let success = flow.nodes.iter().find(|node| node.concept == Concept::Break).unwrap();
    assert!(success.flavors.iter().any(|flavor| flavor == "break-value-binding:lease"));
    let standby = flow.nodes.iter().find(|node| node.code.starts_with("sleep()")).unwrap();
    let failure = flow.nodes.iter().find(|node| node.concept == Concept::Return && node.code.contains("return Err(error)")).unwrap();
    let after = flow.nodes.iter().find(|node| node.code.starts_with("log(lease)")).unwrap();
    let exec = |from: &str, pin: &str, to: &str| flow.edges.iter().any(|edge|
        edge.kind == FlowEdgeKind::Exec && edge.from == from && edge.from_pin == pin && edge.to == to);
    assert!(exec(&loop_node.id, "body", &decision.id));
    assert!(exec(&decision.id, "case:0", &success.id));
    assert!(exec(&decision.id, "case:1", &standby.id));
    assert!(exec(&decision.id, "case:2", &failure.id));
    assert!(exec(&loop_node.id, "out", &after.id));
    assert!(!flow.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Exec && edge.from == standby.id));

    let control = ir.flows.iter().find(|flow| flow.name == "while_control").unwrap();
    let while_node = control.nodes.iter().find(|node| node.concept == Concept::LoopInfinite).unwrap();
    assert_eq!(while_node.label, "repeat forever");
    assert!(!while_node.flavors.iter().any(|flavor| flavor == "rust-loop"));
    assert!(!control.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Repeat the loop body")));
}
