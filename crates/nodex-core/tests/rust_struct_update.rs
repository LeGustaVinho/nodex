use nodex_ir::{Concept, ValueBasis, reading::{ReadingContent, ReadingSite}};
use nodex_lang_rust::lower_source;

fn is_update(site: &ReadingSite) -> bool {
    matches!(&site.content, ReadingContent::Detail { label, .. } if label == "Fill omitted fields from the base struct")
}

#[test]
fn struct_updates_explain_the_base_at_the_exact_dot_dot_token() {
    let source = r#"
        #[derive(Clone)]
        struct Event { tick: u32, region: u8, name: String }

        fn emit_cosmetic(event: Event, tick: u32) -> Event {
            Event { tick, ..event }
        }

        fn combine(cosmetic: &Event, ticks: &mut impl Iterator<Item = u32>) -> Event {
            Event { region: ticks.next().unwrap() as u8, tick: ticks.next().unwrap(), .. /* base */ cosmetic.clone() }
        }

        fn no_update(event: Event) -> (u32, std::ops::RangeFrom<u32>) {
            let Event { tick, .. } = event;
            let complete = Event { tick: 9, region: 2, name: String::new() };
            (complete.tick, tick..)
        }
    "#;
    let ir = lower_source("src/lib.rs", source);

    for (function, following, base) in [
        ("emit_cosmetic", "..event", "event"),
        ("combine", ".. /* base */ cosmetic.clone()", "cosmetic.clone()"),
    ] {
        let flow = ir.flows.iter().find(|flow| flow.name == function).unwrap();
        let mut sites = flow.reading.iter().flatten().filter(|site| is_update(site));
        let site = sites.next().unwrap_or_else(|| panic!("Missing struct update in {function}"));
        assert!(sites.next().is_none());
        let start = source.find(following).unwrap();
        assert_eq!((site.span.start_byte as usize, site.span.end_byte as usize), (start, start + 2));
        assert_eq!(site.basis, ValueBasis::Syntax);
        assert_eq!(site.path.as_deref(), Some("0"));
        assert_eq!(flow.node(&site.node).unwrap().expr.as_ref().unwrap().concept, Concept::Construct);
        let ReadingContent::Detail { text, .. } = &site.content else { unreachable!() };
        assert!(text.contains(&format!("in source order, then evaluate {base} as the base")), "{text}");
        assert!(text.contains("takes each omitted field from that base"), "{text}");
        assert!(text.contains("implements Copy, or moves it into the new struct otherwise"), "{text}");
    }

    let no_update = ir.flows.iter().find(|flow| flow.name == "no_update").unwrap();
    assert!(!no_update.reading.iter().flatten().any(is_update));
}
