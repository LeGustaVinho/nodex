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

#[test]
fn embedded_match_values_explain_arms_without_execution_pins() {
    let source = r#"
        fn embedded(profile: Option<i32>) -> Option<i32> {
            let fraction = match profile {
                Some(value) if value > 0 => { value / 2 },
                Some(value) => value,
                None => { return None; },
            };
            Some(fraction)
        }
        fn statement(profile: Option<i32>) -> i32 {
            match profile { Some(value) => value, None => 0 }
        }
        fn unit(value: bool) {
            let _ = match value { true => { 5; }, false => () };
        }
        fn spaces() -> &'static str {
            let label = match "keep  range" { "keep  range" => "double  space", _ => "other" };
            label
        }
        fn fallback(input: Option<i32>, profile: Option<i32>) -> i32 {
            let Some(value) = input else {
                match profile { Some(other) => return other, None => return 0 }
            };
            value
        }
        fn wide(profile: Option<f64>, elapsed: f64) -> f64 {
            let fraction = match profile {
                Some(value) => value,
                None => f64::min(
                    1.0,
                    elapsed / 10.0
                        + 0.2
                        + 0.3,
                ),
            };
            fraction
        }
        fn labelled(flag: bool) -> i32 {
            let value = match flag { true => 'arm: { break 'arm 7; }, false => 0 };
            value
        }
    "#;
    let ir = lower_source("src/lib.rs", source);
    let embedded = ir.flows.iter().find(|flow| flow.name == "embedded").unwrap();
    let paths = embedded.reading.iter().flatten().find(|site| matches!(&site.content,
        ReadingContent::Paths { input, .. } if input.starts_with("Compare profile"))).unwrap();
    assert_eq!(snippet(source, paths), "match");
    assert_eq!(paths.path.as_deref(), Some("0"));
    let ReadingContent::Paths { branches, .. } = &paths.content else { unreachable!() };
    assert_eq!(branches.len(), 3);
    assert!(branches.iter().all(|branch| branch.pin.is_none()));
    assert_eq!(branches[0].label, "Arm 1 · Some(value) if value > 0");
    assert_eq!(branches[0].output, "value / 2");
    assert!(branches[0].action.contains("completes normally"));
    assert_eq!(branches[1].output, "value");
    assert_eq!(branches[2].output, "return None");
    assert!(branches[2].action.contains("before the match produces a value"));

    let statement = ir.flows.iter().find(|flow| flow.name == "statement").unwrap();
    assert!(statement.nodes.iter().any(|node| node.outputs.iter().any(|pin| pin.id == "case:0")));
    assert!(statement.reading.iter().flatten().all(|site| !matches!(site.content, ReadingContent::Paths { .. })));

    let unit = ir.flows.iter().find(|flow| flow.name == "unit").unwrap();
    let unit_branches = unit.reading.iter().flatten().find_map(|site| match &site.content {
        ReadingContent::Paths { branches, .. } => Some(branches),
        _ => None,
    }).unwrap();
    assert_eq!(unit_branches[0].output, "()");
    assert!(unit_branches[0].action.contains("its block reaches the end"));

    let spaces = ir.flows.iter().find(|flow| flow.name == "spaces").unwrap();
    let (input, arms) = spaces.reading.iter().flatten().find_map(|site| match &site.content {
        ReadingContent::Paths { input, branches, .. } => Some((input, branches)),
        _ => None,
    }).unwrap();
    assert_eq!(input, "Compare \"keep  range\" with the arm patterns in order");
    assert_eq!(arms[0].label, "Arm 1 · \"keep  range\"");
    assert_eq!(arms[0].output, "\"double  space\"");

    let fallback = ir.flows.iter().find(|flow| flow.name == "fallback").unwrap();
    assert!(fallback.nodes.iter().any(|node| node.concept == Concept::Match));
    let fallback_inputs: Vec<_> = fallback.reading.iter().flatten().filter_map(|site| match &site.content {
        ReadingContent::Paths { input, .. } => Some(input.as_str()),
        _ => None,
    }).collect();
    assert_eq!(fallback_inputs, ["Check whether input matches Some(value)"]);

    let wide = ir.flows.iter().find(|flow| flow.name == "wide").unwrap();
    let multiline = wide.reading.iter().flatten().find_map(|site| match &site.content {
        ReadingContent::Paths { branches, .. } => Some(&branches[1].output),
        _ => None,
    }).unwrap();
    assert!(multiline.len() > 80 && multiline.contains("+ 0.3,"));
    assert!(multiline.ends_with(')'));

    let labelled = ir.flows.iter().find(|flow| flow.name == "labelled").unwrap();
    let branch = labelled.reading.iter().flatten().find_map(|site| match &site.content {
        ReadingContent::Paths { branches, .. } => branches.first(),
        _ => None,
    }).unwrap();
    assert_eq!(branch.output, "'arm: { break 'arm 7; }");
    assert!(branch.action.contains("this labeled block completes"));
}

#[test]
fn embedded_if_values_explain_the_selected_result_and_the_missing_else() {
    let source = r#"
        fn choices(flag: bool) -> i32 {
            let observed = if flag { 10 } else { return 20; };
            let no_else = if flag { 5; };
            observed
        }
        fn statement(flag: bool) -> i32 {
            if flag { return 30; }
            40
        }
    "#;
    let ir = lower_source("src/lib.rs", source);
    let choices = ir.flows.iter().find(|flow| flow.name == "choices").unwrap();
    let paths: Vec<_> = choices.reading.iter().flatten().filter_map(|site| match &site.content {
        ReadingContent::Paths { branches, .. } => Some((site, branches)),
        _ => None,
    }).collect();
    let [(first, branches), (second, unit)] = paths.as_slice() else { panic!("{paths:?}") };
    for site in [first, second] {
        assert_eq!(snippet(source, site), "if");
        assert_eq!(site.path.as_deref(), Some("0"));
        assert_eq!(choices.node(&site.node).unwrap().expr.as_ref().unwrap().concept, Concept::Choose);
    }
    assert_eq!(branches[0].output, "10");
    assert_eq!(branches[1].output, "return 20");
    assert!(branches[1].action.contains("before the if expression produces a value"));
    assert_eq!(unit[1].output, "()");
    assert!(unit[1].action.contains("if it completes"));
    assert!(branches.iter().chain(unit.iter()).all(|branch| branch.pin.is_none()));

    let statement = ir.flows.iter().find(|flow| flow.name == "statement").unwrap();
    assert!(statement.nodes.iter().any(|node| node.concept == Concept::Branch));
    assert!(statement.reading.iter().flatten().all(|site| !matches!(site.content, ReadingContent::Paths { .. })));
}
