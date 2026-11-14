use nodex_ir::{Concept, FlowEdgeKind, ValueBasis, reading::ReadingContent};
use nodex_lang_rust::lower_source;

#[test]
fn loop_header_explains_claim_retry_without_adding_an_execution_edge() {
    let source = r#"
enum ClaimError { Held, Other }

async fn claim(standby: bool) -> Result<u64, ClaimError> {
    let owner = format!("claim");
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
    assert!(standby.flavors.contains(&format!("loop-repeat:{}", loop_node.id)));
    assert!(!success.flavors.iter().chain(&failure.flavors).any(|flavor| flavor.starts_with("loop-repeat:")));

    let control = ir.flows.iter().find(|flow| flow.name == "while_control").unwrap();
    let while_node = control.nodes.iter().find(|node| node.concept == Concept::LoopInfinite).unwrap();
    assert_eq!(while_node.label, "repeat forever");
    assert!(!while_node.flavors.iter().any(|flavor| flavor == "rust-loop"));
    assert!(!control.reading.iter().flatten().any(|site| matches!(&site.content,
        ReadingContent::Detail { label, .. } if label == "Repeat the loop body")));
}

#[test]
fn loop_retry_follows_normal_completion_and_preserves_cleanup() {
    let source = r#"
struct Tracked;
impl Drop for Tracked {
    fn drop(&mut self) { log("drop"); }
}

async fn commit(mut wait: u64) -> bool {
    loop {
        match flush().await {
            Ok(()) => return true,
            Err(Error::Fenced) => return false,
            Err(error) => {
                eprintln!("retry: {error}");
                sleep(wait).await;
                wait = (wait * 2).min(100);
            }
        }
    }
}

fn cleanup(flag: bool) {
    loop {
        let _iteration: Tracked = Tracked;
        match flag {
            true => {
                let _branch: Tracked = Tracked;
                work();
            }
            false => return,
        }
    }
}

fn later(flag: bool) {
    loop {
        match flag {
            true => work(),
            false => break,
        }
        later_work();
    }
}

fn dead_join(flag: bool) {
    loop {
        match flag {
            true => { return; dead_one(); dead_two(); }
            false => { break; dead_one(); dead_two(); }
        }
        dead_three();
    }
}

fn dead_loop() {
    return;
    loop { dead(); }
}

fn inner_loop() {
    loop { loop {} }
}

fn hidden_return() {
    loop { let _ = return; }
}

fn unsafe_return() {
    loop { unsafe { return; } }
}

fn hidden_block() {
    loop { let _ = { return; }; }
}

fn diverging_guard(flag: bool) {
    loop { match flag { _ if { return; } => work(), _ => work() } }
}

fn dead_branch() {
    loop { if false { work(); } else { return; } }
}

fn deferred_async_value() {
    loop { let _future = async { return; }; }
}

"#;
    let ir = lower_source("src/lib.rs", source);
    let find = |name| ir.flows.iter().find(|flow| flow.name == name).unwrap();
    let commit = find("commit");
    let header = commit.nodes.iter().find(|node| node.concept == Concept::LoopInfinite).unwrap();
    let backoff = commit.nodes.iter().find(|node| node.code.starts_with("wait =")).unwrap();
    assert!(backoff.flavors.contains(&format!("loop-repeat:{}", header.id)));
    assert!(commit.nodes.iter().filter(|node| node.concept == Concept::Return)
        .all(|node| !node.flavors.iter().any(|flavor| flavor.starts_with("loop-repeat:"))));

    let cleanup = find("cleanup");
    let header = cleanup.nodes.iter().find(|node| node.concept == Concept::LoopInfinite).unwrap();
    let body_drop = cleanup.nodes.iter().find(|node| node.code == "} // drop(_iteration)"
        && node.flavors.contains(&format!("loop-repeat:{}", header.id))).unwrap();
    let branch_drop = cleanup.nodes.iter().find(|node| node.code == "} // drop(_branch)").unwrap();
    assert!(branch_drop.flavors.contains(&format!("loop-cleanup:{}", body_drop.id)));
    assert!(!cleanup.nodes.iter().find(|node| node.code.starts_with("work()")).unwrap()
        .flavors.iter().any(|flavor| flavor.starts_with("loop-")));

    let later = find("later");
    let repeats: Vec<_> = later.nodes.iter().filter(|node| node.flavors.iter().any(|flavor| flavor.starts_with("loop-repeat:"))).collect();
    assert_eq!(repeats.len(), 1);
    assert!(repeats[0].code.starts_with("later_work()"));
    assert!(find("deferred_async_value").nodes.iter().any(|node| node.flavors.iter().any(|flavor| flavor.starts_with("loop-repeat:"))));
    for name in ["dead_join", "dead_loop", "inner_loop", "hidden_return", "unsafe_return", "hidden_block", "diverging_guard", "dead_branch"] {
        assert!(find(name).nodes.iter().all(|node| !node.flavors.iter().any(|flavor|
            flavor.starts_with("loop-repeat:") || flavor.starts_with("loop-cleanup:"))), "{name}");
    }
}

#[test]
fn loop_retry_omits_opaque_macro_endpoints() {
    let source = r#"
macro_rules! escape { () => { return } }
macro_rules! eprintln { ($($tt:tt)*) => { return } }
fn macro_initializer() {
    loop { let _ = escape!(); }
}
fn macro_shadow() {
    loop { eprintln!("escape"); }
}
"#;
    let ir = lower_source("src/lib.rs", source);
    assert!(ir.flows.iter().flat_map(|flow| &flow.nodes).all(|node| !node.flavors.iter().any(|flavor|
        flavor.starts_with("loop-repeat:") || flavor.starts_with("loop-cleanup:"))));
}
