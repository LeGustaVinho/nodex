use nodex_ir::{Concept, Expr, FlowEdgeKind, Span, ValueBasis, reading::ReadingContent};
use nodex_lang_rust::lower_source;

fn snippet<'a>(source: &'a str, span: &Span) -> &'a str {
    &source[span.start_byte as usize..span.end_byte as usize]
}

#[test]
fn tuple_assignment_shows_the_right_side_before_writing_each_existing_field() {
    let source = r#"
struct Events { events: Vec<u32>, cosmetics: Vec<u32> }

fn load_phase(encounter: &mut Events, setup: Result<Events, ()>) -> Result<(), ()> {
    let setup = setup?;
    (encounter.events, encounter.cosmetics) = (setup.events, setup.cosmetics);
    Ok(())
}

fn swap_or_leave(a: &mut u32, b: &mut u32, values: Result<(u32, u32), ()>) -> Result<(), ()> {
    (*a, *b) = values?;
    (*a, *b) = (*b, *a);
    Ok(())
}
"#;
    let ir = lower_source("src/lib.rs", source);
    for (name, right, destinations) in [
        ("load_phase", "(setup.events, setup.cosmetics)", ["encounter.events", "encounter.cosmetics"]),
        ("swap_or_leave", "values?", ["*a", "*b"]),
    ] {
        let flow = ir.flows.iter().find(|flow| flow.name == name).unwrap();
        let node = flow.nodes.iter().find(|node| node.concept == Concept::Destructure && node.code.contains(right)).unwrap();
        let site = flow.reading.iter().flatten().find(|site| site.node == node.id && matches!(&site.content,
            ReadingContent::Steps { label, .. } if label == "Assign parts to existing destinations")).unwrap();
        assert_eq!(site.basis, ValueBasis::Syntax);
        assert!(site.path.is_some());
        assert_eq!(snippet(source, &site.span), "=");
        let ReadingContent::Steps { steps, .. } = &site.content else { unreachable!() };
        assert_eq!(steps.iter().map(|step| snippet(source, &step.span)).collect::<Vec<_>>(),
            [right, destinations[0], destinations[1]]);
        assert_eq!(steps[0].timing, "First");
        assert!(steps[0].action.contains("If evaluation exits early"));
        assert!(steps[1..].iter().all(|step| step.action.contains("drop its previous value if initialized")));
        assert!(flow.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Exec && edge.from == node.id));
        if name == "swap_or_leave" {
            assert!(flow.reading.iter().flatten().any(|site| site.node == node.id && snippet(source, &site.span) == "?"));
        }
    }
}

#[test]
fn assignee_rest_and_wildcard_are_distinct_from_a_range_or_a_binding_pattern() {
    let source = r#"
struct Pair { head: u32, tail: u32 }

fn assignments(mut output: [u32; 2], array: [u32; 4], pair: Pair, name: &mut u32) {
    let mut first = 0;
    let mut last = 0;
    [first, _, .., last] = array;
    (/* grouped */ (output[..][0], _)) = (last, first);
    Pair { head: *name, tail: last } = pair;
    *name = last;
    let [head, ..] = output;
    let updated = Pair { head: 1, ..pair };
    let range = ..;
}
"#;
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "assignments").unwrap();
    let cards = flow.reading.iter().flatten().filter_map(|site| match &site.content {
        ReadingContent::Steps { label, steps } if label == "Assign parts to existing destinations" => Some(steps),
        _ => None,
    }).collect::<Vec<_>>();
    assert_eq!(cards.len(), 3);
    assert_eq!(cards[0].iter().map(|step| snippet(source, &step.span)).collect::<Vec<_>>(), ["array", "first", "_", "..", "last"]);
    assert_eq!(cards[0][2].timing, "Ignore one part");
    assert_eq!(cards[0][3].timing, "Ignore the remaining parts");
    assert_eq!(cards[1].iter().map(|step| snippet(source, &step.span)).collect::<Vec<_>>(), ["(last, first)", "output[..][0]", "_"]);
    assert_eq!(cards[2].iter().map(|step| snippet(source, &step.span)).collect::<Vec<_>>(), ["pair", "*name", "last"]);
    assert!(flow.reading.iter().flatten().any(|site| matches!(&site.content, ReadingContent::Detail { label, .. }
        if label == "Fill omitted fields from the base struct")));
    let rest = cards[0][3].span.start_byte;
    let index = source.find("output[..]").unwrap() as u32 + "output[".len() as u32;
    let range = source.find("range = ..").unwrap() as u32 + "range = ".len() as u32;
    let ranges = flow.reading.iter().flatten().filter_map(|site| match &site.content {
        ReadingContent::Detail { label, .. } if label == "Range has no bounds" => Some(site.span.start_byte),
        _ => None,
    }).collect::<Vec<_>>();
    assert_eq!(ranges, [index, range]);

    fn sites(expr: &Expr, ranges: &mut Vec<u32>) {
        ranges.extend(expr.syntax_sites.iter().flatten().filter(|site| site.key == "range-open").map(|site| site.span.start_byte));
        for child in &expr.children {
            sites(child, ranges);
        }
    }
    let mut syntax_ranges = Vec::new();
    for node in &flow.nodes {
        syntax_ranges.extend(node.syntax_sites.iter().flatten().filter(|site| site.key == "range-open").map(|site| site.span.start_byte));
        if let Some(expr) = &node.expr {
            sites(expr, &mut syntax_ranges);
        }
    }
    assert!(!syntax_ranges.contains(&rest));
    assert!(syntax_ranges.contains(&index));
    assert!(syntax_ranges.contains(&range));
}
