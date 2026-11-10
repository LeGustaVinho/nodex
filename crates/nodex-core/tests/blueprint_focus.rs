use nodex_core::semantic::{Locator, enrich_flow};
use nodex_ir::semantic::{ByteRange, EmitFocus, RefKind, SemanticFacts, SemanticRef, SemanticTarget};
use nodex_ir::{Concept, FlowGraph, FlowNode, Focus, stable_id};

fn statement<'a>(graph: &'a FlowGraph, code: &str) -> &'a FlowNode {
    graph.nodes.iter().find(|n| n.code.starts_with(code)).unwrap_or_else(|| panic!("no statement starting with {code:?}"))
}

fn focus(value: &Option<Focus>, primary: &str, prefix: Option<&str>) {
    let actual = value.as_ref().expect("focus");
    assert_eq!(actual.primary, primary);
    assert_eq!(actual.prefix.as_deref(), prefix);
}

#[test]
fn blueprint_focus_marks_the_written_target_even_when_the_name_repeats() {
    let csharp = "class C { void Run() { café.Value = café.Value + 1; } }";
    let typescript = "function run() { café.Value = café.Value + 1; }";
    let rust = "fn run() { café.Value = café.Value + 1; }";
    for (source, ir) in [
        (csharp, nodex_lang_csharp::lower_source("Test.cs", csharp)),
        (typescript, nodex_lang_typescript::lower_source("test.ts", typescript)),
        (rust, nodex_lang_rust::lower_source("test.rs", rust)),
    ] {
        let flow = ir.flows.iter().find(|flow| flow.name.eq_ignore_ascii_case("run")).unwrap();
        let node = statement(flow, "café.Value =");
        let target = node.focus.as_ref().unwrap();
        let [start, end] = target.primary_bytes.unwrap().map(|byte| byte as usize);
        assert_eq!(&source[start..end], "Value");
        assert_eq!(start, source.find("Value").unwrap(), "{source}");
        let mut old = serde_json::to_value(target).unwrap();
        old.as_object_mut().unwrap().remove("primary_bytes");
        assert!(serde_json::from_value::<Focus>(old).unwrap().primary_bytes.is_none());
    }
}

#[test]
fn csharp_focus_follows_the_written_operand() {
    let ir = nodex_lang_csharp::lower_source("Unit.cs", r#"
class Unit {
    async Task Exercise() {
        body.AddForce(Inner());
        renderer.enabled = Compute();
        items[i] = Compute();
        var value = renderer.enabled;
        var item = items[i];
        var cast = (Widget)item;
        var created = new Widget();
        var qualified = new System.Collections.Generic.List<int>();
        var converted = (System.Collections.Generic.List<int>)item;
        var tested = item is System.Collections.Generic.List<int> list;
        var choice = ready && count > 0 ? item : null;
        var matches = item is Widget other;
        Changed += Handler;
        this.Changed?.Invoke();
        await Work();
        Task.Run(Work);
        Task.Run(cancellationToken: ct, action: Work);
        lock (gate) { Work(); }
        if (ready && count > 0) { return; }
        while (ready && count > 0) { break; }
        for (int i = 0; i < count; i++) { Work(); }
        foreach (var item in items.Values) { Work(); }
        switch (item.Kind) { default: break; }
        try { throw new Exception("é <tag>"); } catch { throw; }
    }
}
"#);
    let graph = ir.flows.iter().find(|g| g.name == "Exercise").unwrap();
    let outer = statement(graph, "body.AddForce");
    focus(&outer.focus, "AddForce", Some("body."));
    let root = outer.expr.as_ref().unwrap();
    assert_eq!(root.focus, outer.focus);
    let inner = root.children.iter().find(|e| e.concept == Concept::Call).unwrap();
    focus(&inner.focus, "Inner", None);
    assert_eq!(outer.id, stable_id(&graph.symbol, Concept::Call, "body.AddForce(Inner());", 0));

    for (code, primary, prefix) in [
        ("renderer.enabled =", "enabled", Some("renderer.")),
        ("items[i] =", "items[i]", None),
        ("var value =", "value", None),
        ("Changed +=", "Changed", None),
        ("this.Changed?.Invoke", "Changed", Some("this.")),
        ("await Work", "Work()", None),
        ("Task.Run", "Work", None),
        ("Task.Run(cancellationToken:", "Work", None),
        ("lock (", "gate", None),
        ("if (", "ready && count > 0", None),
        ("while (", "ready && count > 0", None),
        ("for (", "i < count", None),
        ("foreach (", "items.Values", None),
        ("switch (", "item.Kind", None),
        ("throw new", r#"new Exception("é <tag>")"#, None),
    ] {
        focus(&statement(graph, code).focus, primary, prefix);
    }
    for (code, primary, prefix) in [
        ("var value =", "enabled", Some("renderer.")),
        ("var item =", "items[i]", None),
        ("var cast =", "Widget", None),
        ("var created =", "Widget", None),
        ("var qualified =", "List<int>", Some("System.Collections.Generic.")),
        ("var converted =", "List<int>", Some("System.Collections.Generic.")),
        ("var tested =", "List<int> list", Some("System.Collections.Generic.")),
        ("var choice =", "ready && count > 0", None),
        ("var matches =", "Widget other", None),
    ] {
        focus(&statement(graph, code).expr.as_ref().unwrap().focus, primary, prefix);
    }
    for code in ["return;", "break;", "try", "throw;"] {
        assert!(statement(graph, code).focus.is_none(), "{code}");
    }

    let mut old = serde_json::to_value(outer).unwrap();
    old.as_object_mut().unwrap().remove("focus");
    old["expr"].as_object_mut().unwrap().remove("focus");
    let legacy: FlowNode = serde_json::from_value(old).unwrap();
    assert_eq!(legacy.id, outer.id);
    assert!(legacy.focus.is_none() && legacy.expr.unwrap().focus.is_none());
}

#[test]
fn roslyn_raises_focus_the_event_without_reclassifying_the_outer_call() {
    let ir = nodex_lang_csharp::lower_source("Unit.cs", r#"
class Unit {
    event System.Func<int> Saved;
    void Exercise() { Saved(); Process(Saved()); }
}
"#);
    let graph = ir.flows.iter().find(|g| g.name == "Exercise").unwrap();
    let saved = statement(graph, "Saved()").expr.as_ref().unwrap();
    let nested = &statement(graph, "Process(").expr.as_ref().unwrap().children[0];
    let origins = &ir.symbols.iter().find(|s| s.name == "Saved").unwrap().origins;
    let range = |span: &nodex_ir::Span| ByteRange { file: span.file.clone(), start: span.start_byte, end: span.end_byte };
    let target = SemanticTarget {
        display: "Unit.Saved".into(), kind: "event".into(), decl: Some(range(&origins[0])),
        container: Some("Unit".into()), external: false, linq: false,
    };
    let facts = SemanticFacts {
        refs: [saved, nested].map(|e| SemanticRef { site: range(&e.span), kind: RefKind::Raise, target: target.clone(), emit_focus: None }).to_vec(),
        ..Default::default()
    };
    let enriched = enrich_flow(graph, &facts, &Locator::new(&ir.symbols));
    let emit = statement(&enriched, "Saved()");
    assert_eq!(emit.concept, Concept::Emit);
    assert_eq!(emit.id, statement(graph, "Saved()").id);
    focus(&emit.focus, "Saved", None);
    assert_eq!(emit.expr.as_ref().unwrap().target.as_deref(), Some("Unit.Saved"));
    let call = statement(&enriched, "Process(");
    assert_eq!(call.concept, Concept::Call);
    focus(&call.focus, "Process", None);
    assert_eq!(call.expr.as_ref().unwrap().children[0].target.as_deref(), Some("Unit.Saved"));
}

#[test]
fn typescript_events_waits_and_control_flow_show_their_own_operands() {
    let condition = (0..80).map(|i| format!("ready{i}")).collect::<Vec<_>>().join(" && ");
    let ir = nodex_lang_typescript::lower_source("unit.ts", &format!(r#"
async function exercise() {{
    outer(inner());
    renderer.enabled = compute();
    bus.on("saved", handler);
    bus.on("changed", () => handler());
    bus.emit("saved", value);
    state.saved.emit(value);
    stream.subscribe(x => log(x));
    await sleep(50);
    await new Promise(resolve => setTimeout(resolve, duration()));
    await new Promise<void>(resolve => {{
        setTimeout(resolve, delayMs);
        console.log("queued", 99);
    }});
    await new Promise(resolve => console.log("setTimeout", 99));
    queueMicrotask(handler);
    if ({condition}) {{ return; }}
    while (ready && count > 0) {{ break; }}
    for (let i = 0; i < count; i++) {{ step(); }}
    for (const item of items.values()) {{ step(); }}
    switch (value.kind) {{ default: break; }}
    const typed = value as Widget;
    const checked = value instanceof Widget;
    const qualified = new Models.Widgets.List<number>();
    throw new Error("<tag>");
}}
"#));
    let graph = ir.flows.iter().find(|g| g.name == "exercise").unwrap();
    for (code, primary, prefix) in [
        ("outer(", "outer", None),
        ("renderer.enabled =", "enabled", Some("renderer.")),
        ("bus.on(\"saved\"", r#""saved""#, None),
        ("bus.on(\"changed\"", r#""changed""#, None),
        ("bus.emit(", r#""saved""#, None),
        ("state.saved.emit(", "saved", Some("state.")),
        ("stream.subscribe(", "stream", None),
        ("await sleep(", "50", None),
        ("await new Promise", "duration()", None),
        ("await new Promise<void>", "delayMs", None),
        ("await new Promise(resolve => console.log", "new Promise(resolve => console.log(\"setTimeout\", 99))", None),
        ("queueMicrotask(", "handler", None),
        ("if (", &condition, None),
        ("while (", "ready && count > 0", None),
        ("for (", "i < count", None),
        ("for (const item", "items.values()", None),
        ("switch (", "value.kind", None),
        ("throw ", r#"new Error("<tag>")"#, None),
    ] {
        focus(&statement(graph, code).focus, primary, prefix);
    }
    for code in ["const typed", "const checked"] {
        focus(&statement(graph, code).expr.as_ref().unwrap().focus, "Widget", None);
    }
    focus(&statement(graph, "const qualified").expr.as_ref().unwrap().focus, "List<number>", Some("Models.Widgets."));
    assert_eq!(statement(graph, "await new Promise<void>").label, "wait delayMs");
    assert_eq!(statement(graph, "await new Promise(resolve => console.log").concept, Concept::Await);
}

#[test]
fn javascript_and_rust_keep_unicode_and_literal_source() {
    let js = nodex_lang_typescript::lower_source("unit.js", r#"
function exercise() {
    objeto?.método(inner());
    element.value = "<img>  🐈";
    return items[i];
}
"#);
    let graph = &js.flows[0];
    focus(&statement(graph, "objeto").focus, "método", Some("objeto?."));
    focus(&statement(graph, "element").focus, "value", Some("element."));
    focus(&statement(graph, "return").focus, "items[i]", None);

    let rust = nodex_lang_rust::lower_source("unit.rs", r#"
async fn exercise() {
    let naïve = Widget::new();
    let answer = if ready { touch(); 2 } else { touch(); 1 };
    items[i] = compute();
    io.outer(inner());
    if ready && count > 0 { return; }
    while ready && count > 0 { break; }
    for n in 0..count { step(n); }
    match value.kind { _ => step(0), }
    task().await;
    tokio::time::sleep(delay).await;
    std::thread::spawn(worker);
    io.tx.send(value).unwrap();
    panic!("<img>  🐈");
}
"#);
    let graph = &rust.flows[0];
    let local = statement(graph, "let naïve");
    focus(&local.focus, "naïve", None);
    focus(&local.expr.as_ref().unwrap().focus, "Widget", None);
    let assignments: Vec<_> = graph.nodes.iter()
        .filter(|n| n.concept == Concept::Assign && n.flavors.iter().any(|f| f == "block-value"))
        .collect();
    assert_eq!(assignments.len(), 2);
    for assignment in assignments {
        focus(&assignment.focus, "answer", None);
    }
    for (code, primary, prefix) in [
        ("items[i] =", "items[i]", None),
        ("io.outer(", "outer", Some("io.")),
        ("if ready &&", "ready && count > 0", None),
        ("while ", "ready && count > 0", None),
        ("for ", "0..count", None),
        ("match ", "value.kind", None),
        ("task().await", "task()", None),
        ("tokio::time::sleep", "delay", None),
        ("std::thread::spawn", "worker", None),
        ("io.tx.send(", "tx", Some("io.")),
        ("panic!", r#""<img>  🐈""#, None),
    ] {
        focus(&statement(graph, code).focus, primary, prefix);
    }
}

#[test]
fn semantic_call_sites_keep_conditional_receivers_and_event_roles() {
    let cs = nodex_lang_csharp::lower_source("Calls.cs", "class Calls { void Run() { receiver?.Outer(Inner()); } }");
    let graph = &cs.flows[0];
    let outer = statement(graph, "receiver?.Outer").expr.as_ref().unwrap();
    let inner = &outer.children[0];
    let call = |e: &nodex_ir::Expr, start, name: &str, emit_focus| SemanticRef {
        site: ByteRange { file: e.span.file.clone(), start, end: e.span.end_byte },
        kind: RefKind::Call,
        target: SemanticTarget {
            decl: None, display: name.to_owned(), container: None, kind: "method".into(), external: true, linq: false,
        },
        emit_focus,
    };
    let facts = SemanticFacts {
        refs: vec![call(outer, outer.span.start_byte + "receiver?".len() as u32, "Calls.Outer(int)", None),
                   call(inner, inner.span.start_byte, "Calls.Inner()", None)],
        ..Default::default()
    };
    let result = enrich_flow(graph, &facts, &Locator::new(&cs.symbols));
    let expr = statement(&result, "receiver?.Outer").expr.as_ref().unwrap();
    assert_eq!(expr.target.as_deref(), Some("Calls.Outer(int)"));
    assert_eq!(expr.children[0].target.as_deref(), Some("Calls.Inner()"));

    let ts = nodex_lang_typescript::lower_source("Mixed.ts", r#"
        import { EventEmitter } from "node:events";
        function run() { this.saved.emit("payload  🐈"); bus.emit(event); }
    "#);
    let graph = ts.flows.iter().find(|g| g.name == "run").unwrap();
    let saved = statement(graph, "this.saved").expr.as_ref().unwrap();
    let bus = statement(graph, "bus.emit").expr.as_ref().unwrap();
    focus(&statement(graph, "bus.emit").focus, "event", None);
    let facts = SemanticFacts {
        refs: vec![call(saved, saved.span.start_byte, "EventEmitter.emit(T)", Some(EmitFocus { receiver: true, text: "this.saved".into() })),
                   call(bus, bus.span.start_byte, "EventEmitter.emit(string | symbol)", Some(EmitFocus { receiver: false, text: "event".into() }))],
        ..Default::default()
    };
    let result = enrich_flow(graph, &facts, &Locator::new(&ts.symbols));
    focus(&statement(&result, "this.saved").focus, "saved", Some("this."));
    focus(&statement(&result, "bus.emit").focus, "event", None);
}

#[test]
fn long_chain_qualifiers_are_bounded_and_reference_the_written_bytes() {
    let receiver = "ê".repeat(80);
    let source = format!("function chain() {{ {receiver}{}; }}", ".map(x => x)".repeat(200));
    let ir = nodex_lang_typescript::lower_source("chain.ts", &source);
    let call = &ir.flows[0].nodes[1];
    let root = call.expr.as_ref().unwrap();
    assert_eq!(root.concept, Concept::Pipeline);
    for stage in &root.children[1..] {
        let focus = stage.focus.as_ref().unwrap();
        let [start, end] = focus.prefix_bytes.expect("long qualifier");
        let qualifier = &source[start as usize..end as usize];
        assert!(qualifier.starts_with(&receiver) && qualifier.ends_with('.'));
        let preview = focus.prefix.as_deref().unwrap();
        assert!(preview.len() <= 99 && qualifier.starts_with(preview.trim_end_matches('…')));
    }
    assert_eq!(call.focus, root.children.last().unwrap().focus);
}
#[test]
fn csharp_blueprint_marks_argument_directions() {
    let ir = nodex_lang_csharp::lower_source("Arguments.cs", r#"
        class Example {
            void Run(int input, int count) {
                Read(in input, ref count, out var result);
                Read(in input, ref count, out result);
            }
        }
    "#);
    let flow = ir.flows.iter().find(|flow| flow.name == "Run").unwrap();
    let call = flow.nodes.iter().find_map(|node| node.expr.as_ref()).unwrap();
    for kind in ["argument:in", "argument:ref", "argument:out"] {
        assert!(call.children.iter().any(|expr| expr.flavors.iter().any(|f| f == kind)), "{kind}");
    }
    let call = statement(flow, "Read(in input, ref count, out result)");
    for pin in ["write:count", "write:result"] {
        assert!(call.outputs.iter().any(|p| p.id == pin), "{pin}");
    }
    assert!(call.inputs.iter().any(|pin| pin.id == "use:count"));
    assert!(!call.inputs.iter().any(|pin| pin.id == "use:result"));
}

#[test]
fn rust_blueprint_names_explicit_propagation_types_and_match_bindings() {
    let ir = nodex_lang_rust::lower_source("paths.rs", r#"
        fn count(value: core::option::Option<i32>) -> core::option::Option<i32> {
            let n = value?;
            match Some(n) { Some(total) => Some(total), None => None }
        }
        fn read(value: core::result::Result<i32, ()>) -> core::result::Result<i32, ()> {
            let n = value?;
            Ok(n)
        }
    "#);
    let count = ir.flows.iter().find(|flow| flow.name == "count").unwrap();
    let read = ir.flows.iter().find(|flow| flow.name == "read").unwrap();
    let propagation = |flow: &FlowGraph, label: &str| {
        assert!(flow.nodes.iter().flat_map(|node| &node.outputs).any(|pin| pin.id == "error" && pin.label == label));
    };
    propagation(count, "If absent: return None");
    propagation(read, "If it fails: return Err");
    assert!(count.nodes.iter().any(|node| node.concept == Concept::Match &&
        node.flavors.iter().any(|f| f == "match-binding:0:total")));
}

