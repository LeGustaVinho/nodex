use nodex_ir::{
    Concept, Expr, FlowGraph, ValueBasis,
    reading::{ReadingContent, ReadingSite},
};
use nodex_lang_rust::lower_source;

fn expression<'a>(flow: &'a FlowGraph, code: &str) -> &'a Expr {
    flow.nodes
        .iter()
        .find(|node| node.code.starts_with(code))
        .and_then(|node| node.expr.as_ref())
        .unwrap_or_else(|| panic!("No expression for {code}"))
}

fn readings<'a>(flow: &'a FlowGraph, label: &'a str) -> impl Iterator<Item = &'a ReadingSite> {
    flow.reading.iter().flatten().filter(move |site| {
        matches!(&site.content, ReadingContent::Detail { label: found, .. } if found == label)
    })
}

fn snippet<'a>(source: &'a str, site: &ReadingSite) -> &'a str {
    &source[site.span.start_byte as usize..site.span.end_byte as usize]
}

#[test]
fn embedded_blocks_show_the_final_value_and_omit_semicolon_terminated_values() {
    let source = r#"
        fn values(flag: bool, mut number: i32) {
            let conditional = { if flag { 1 } else { 2 } };
            let selected = { match number { 0 => 3, _ => 4 } /* after the value */ };
            let discarded = ({ if flag { 5 } else { 6 }; /* after the semicolon */ },);
            let tuple = { (conditional, selected) };
            let assigned = ({ number += 1 },);
            let changed = { let first = 1; first + 1 };
            let progress = match number {
                0 => { selected * 2 },
                _ => conditional,
            };
        }
    "#;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "values").unwrap();
    let conditional = expression(flow, "let conditional");
    let selected = expression(flow, "let selected");
    let discarded = expression(flow, "let discarded");
    assert_eq!(conditional.children[0].concept, Concept::Choose);
    assert_eq!(selected.children[0].concept, Concept::Match);
    assert_eq!(discarded.children[0].concept, Concept::Compose);
    assert!(discarded.children[0].children.is_empty());

    let tails: Vec<_> = readings(flow, "Value from the final expression")
        .map(|site| {
            assert_eq!(site.basis, ValueBasis::Syntax);
            assert!(site.path.is_some());
            snippet(source, site)
        })
        .collect();
    assert_eq!(tails, ["if flag { 1 } else { 2 }", "match number { 0 => 3, _ => 4 }", "(conditional, selected)", "number += 1", "first + 1"]);
    let assigned = readings(flow, "Value from the final expression")
        .find(|site| snippet(source, site) == "number += 1").unwrap();
    assert!(matches!(&assigned.content, ReadingContent::Detail { text, .. } if text.contains("produces () (unit)")));
    let changed = readings(flow, "Value from the final expression")
        .find(|site| snippet(source, site) == "first + 1").unwrap();
    assert!(flow.node(&changed.node).unwrap().flavors.iter().any(|flavor| flavor == "block-value"));

    let mut arms = readings(flow, "Value of this match arm");
    let arm = arms.next().unwrap();
    assert_eq!(snippet(source, arm), "selected * 2");
    assert!(arm.path.as_deref().is_some_and(|path| path.starts_with("0.")));
    assert!(arms.next().is_none());
}

#[test]
fn final_control_expressions_keep_their_function_and_future_boundaries() {
    let source = r#"
        fn direct(flag: bool) -> i32 {
            if flag { 10 } else { 20 }
        }
        fn callback(flag: bool) -> i32 {
            let choose = || { if flag { 30 } else { 40 } };
            choose()
        }
        fn delayed(flag: bool) -> impl std::future::Future<Output = i32> {
            async move { match flag { true => 50, false => 60 } }
        }
        fn forever() -> impl std::future::Future<Output = ()> {
            async { loop {} }
        }
    "#;
    let ir = lower_source("src/lib.rs", source);
    let direct = ir.flows.iter().find(|flow| flow.name == "direct").unwrap();
    assert_eq!(direct.nodes.iter().filter(|node| node.concept == Concept::Return
        && node.flavors.iter().any(|flavor| flavor == "implicit-return")).count(), 2);
    assert!(readings(direct, "Value from the final expression").next().is_none());

    let callback = ir.flows.iter().find(|flow| flow.name == "callback").unwrap();
    let closure = expression(callback, "let choose");
    assert_eq!(closure.concept, Concept::Lambda);
    assert_eq!(closure.children[0].concept, Concept::Choose);
    let closure_result = readings(callback, "Closure result").next().unwrap();
    assert_eq!(snippet(source, closure_result), "if flag { 30 } else { 40 }");

    let delayed = ir.flows.iter().find(|flow| flow.name == "delayed").unwrap();
    let future = expression(delayed, "async move");
    assert!(future.flavors.iter().any(|flavor| flavor == "async"));
    assert!(future.flavors.iter().all(|flavor| flavor != "block"));
    assert_eq!(future.children[0].concept, Concept::Match);
    let result = readings(delayed, "Future result").next().unwrap();
    assert_eq!(snippet(source, result), "match flag { true => 50, false => 60 }");
    assert!(matches!(&result.content, ReadingContent::Detail { text, .. } if text.contains("result of the future")));
    assert!(readings(delayed, "Create a future").next().is_some());

    let forever = ir.flows.iter().find(|flow| flow.name == "forever").unwrap();
    let loop_expression = &expression(forever, "async").children[0];
    assert!(loop_expression.flavors.iter().any(|flavor| flavor == "loop-value"));
    assert!(readings(forever, "Future result").next().is_none());
}
