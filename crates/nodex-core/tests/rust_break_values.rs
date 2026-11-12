use nodex_ir::{Concept, FlowEdgeKind, FlowGraph, FlowNode};
use nodex_lang_rust::lower_source;

fn escape<'a>(flow: &'a FlowGraph, code: &str) -> &'a FlowNode {
    flow.nodes.iter().find(|node| node.concept == Concept::Break && node.code.contains(code))
        .unwrap_or_else(|| panic!("Missing {code} in {}", flow.name))
}

fn destination(node: &FlowNode) -> (Option<&str>, Option<&str>) {
    let flavor = |prefix| node.flavors.iter().find_map(|flavor| flavor.strip_prefix(prefix));
    (flavor("break-value-target:"), flavor("break-value-binding:"))
}

#[test]
fn break_uses_the_pattern_binding_and_initializes_the_loop_binding() {
    let source = r#"
        fn reinforce(input: Option<usize>) -> usize {
            let target = input;
            let target = loop {
                if let Some(target) = target { break target; }
            };
            target
        }
        fn claim(result: Result<Option<i32>, &str>) -> Result<i32, &str> {
            let lease = loop {
                match result {
                    Ok(Some(lease)) => break lease,
                    Ok(None) => continue,
                    Err(error) => return Err(error),
                }
            };
            Ok(lease)
        }
    "#;
    let ir = lower_source("src/lib.rs", source);
    let reinforce = ir.flows.iter().find(|flow| flow.name == "reinforce").unwrap();
    let found = escape(reinforce, "break target");
    assert_eq!(destination(found), (Some("loop"), Some("target₂")));
    let definition = reinforce.nodes.iter().find(|node| node.label.starts_with("target₂ =")).unwrap();
    assert!(definition.outputs.iter().any(|pin| pin.label == "target₂"));
    assert!(reinforce.edges.iter().any(|edge| {
        edge.kind == FlowEdgeKind::Data && edge.to == found.id
            && reinforce.node(&edge.from).unwrap().concept == Concept::Branch
    }));
    assert!(!reinforce.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Exec && edge.from == found.id));

    let claim = ir.flows.iter().find(|flow| flow.name == "claim").unwrap();
    assert_eq!(destination(escape(claim, "break lease")), (Some("loop"), Some("lease")));
    assert!(claim.nodes.iter().any(|node| node.concept == Concept::Continue));
    assert!(claim.nodes.iter().any(|node| node.concept == Concept::Return && node.code.contains("return Err")));
}

#[test]
fn nested_breaks_resolve_the_nearest_eligible_scope_and_its_result() {
    let source = r#"
        fn nested(test: bool) -> i32 {
            let outer = 'scope: loop {
                loop { break 7; };
                let inner = loop {
                    if test { break 'scope 11; }
                    break 13;
                };
                let block_value = 'r#scope: {
                    if test { break 'scope 17; }
                    inner
                };
                break block_value;
            };
            outer
        }
        fn tail() -> Option<i32> {
            loop { break Some(19); }
        }
        fn early(value: Option<i32>) -> Option<i32> {
            let found = 'find: { break 'find value?; };
            Some(found)
        }
        fn normalized(test: bool) -> i32 {
            let outer = 'K: loop {
                let inner = 'K: {
                    if test { break 'K 23; }
                    25
                };
                break inner;
            };
            let accented = 'é: {
                if test { break 'é 27; }
                outer
            };
            accented
        }
    "#;
    let ir = lower_source("src/lib.rs", source);
    let nested = ir.flows.iter().find(|flow| flow.name == "nested").unwrap();
    assert_eq!(destination(escape(nested, "break 7")), (Some("loop"), None));
    assert_eq!(destination(escape(nested, "break 'scope 11")), (Some("loop"), Some("outer")));
    assert_eq!(destination(escape(nested, "break 13")), (Some("loop"), Some("inner")));
    assert_eq!(destination(escape(nested, "break 'scope 17")), (Some("block"), Some("block_value")));
    assert_eq!(destination(escape(nested, "break block_value")), (Some("loop"), Some("outer")));

    let tail = ir.flows.iter().find(|flow| flow.name == "tail").unwrap();
    assert!(escape(tail, "break Some(19)").flavors.iter().any(|flavor| flavor == "break-value-return"));

    let early = ir.flows.iter().find(|flow| flow.name == "early").unwrap();
    let found = escape(early, "break 'find value?");
    assert_eq!(destination(found), (Some("block"), Some("found")));
    assert!(found.outputs.iter().any(|pin| pin.id == "error"));

    let normalized = ir.flows.iter().find(|flow| flow.name == "normalized").unwrap();
    assert_eq!(destination(escape(normalized, "break 'K 23")), (Some("block"), Some("inner")));
    assert_eq!(destination(escape(normalized, "break 'e\u{301} 27")), (Some("block"), Some("accented")));
}
