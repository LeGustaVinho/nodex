use nodex_ir::{Concept, reading::ReadingContent};
use nodex_lang_rust::lower_source;

#[test]
fn while_let_chain_explains_repeated_binding_and_short_circuit() {
    let source = "fn process(a: Option<i32>, condition: impl Fn(i32) -> bool) { while let Some(x) = a && condition(x) { consume(x); } }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "process").unwrap();
    let loop_node = flow.nodes.iter().find(|node| node.concept == Concept::LoopWhile).unwrap();
    let sites: Vec<_> = flow.reading.iter().flatten().filter(|site| site.node == loop_node.id).collect();
    let paths = sites.iter().find_map(|site| match &site.content {
        ReadingContent::Paths { input, branches, timing: Some(timing), .. } if timing == "Before each loop iteration" => Some((input, branches)),
        _ => None,
    }).expect("while chain explanation on loop node");
    assert!(paths.0.contains("On each attempt") && paths.0.contains("in order"));
    assert!(paths.1.iter().any(|branch| branch.action.contains("names bound") && branch.pin.as_deref() == Some("body")));
    assert!(paths.1.iter().any(|branch| branch.action.contains("Skip the remaining checks") && branch.action.contains("end the loop") && branch.pin.as_deref() == Some("out")));
    let steps = sites.iter().find_map(|site| match &site.content {
        ReadingContent::Steps { label, steps } if label == "Check each condition in order" => Some(steps),
        _ => None,
    }).expect("short-circuit explanation on loop node");
    assert!(steps[0].action.contains("a") && steps[0].action.contains("Some(x)") && steps[0].action.contains("later checks"));
    assert!(steps[1].action.contains("condition(x)") && steps[1].timing == "If all earlier checks pass");
    assert!(steps.last().unwrap().action.contains("leave the loop"));
    assert!(sites.iter().any(|site| matches!(&site.content, ReadingContent::Pattern { parts, .. } if parts.iter().any(|part| part.binding.as_deref() == Some("x")))));
}
