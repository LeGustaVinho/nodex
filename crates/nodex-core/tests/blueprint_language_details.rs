use nodex_ir::{Access, BindingForm, FlowEdgeKind, FlowGraph, FlowNode, Pin};

#[test]
fn rust_contract_links_to_source_even_without_syntax_sites() {
    let source = "fn identity(value: i32) -> i32 { value }";
    let mut file = nodex_lang_rust::lower_source("identity.rs", source);
    assert!(file.symbols.iter().all(|symbol| symbol.syntax_sites.is_none()));
    let revision = nodex_core::navigation::source_revision(source);
    file.source_revision = Some(revision.clone());

    let graph = nodex_core::link::link(&[&file], &[]);
    let symbol = graph.symbols.iter().find(|symbol| symbol.name == "identity").unwrap();
    let contract = symbol.rust_contract.as_ref().unwrap();
    let revisions = graph.source_revisions.as_ref().unwrap();
    assert!(!contract.is_empty());
    assert!(contract.iter().all(|item| revisions.get(&item.span.file) == Some(&revision)));
}

fn statement<'a>(graph: &'a FlowGraph, code: &str) -> &'a FlowNode {
    graph.nodes.iter().find(|n| n.code.starts_with(code)).unwrap_or_else(|| panic!("no statement starting with {code}"))
}

fn output<'a>(node: &'a FlowNode, name: &str) -> &'a Pin {
    node.outputs.iter().find(|p| p.id == format!("var:{name}")).unwrap_or_else(|| panic!("no output {name} on {}", node.code))
}

#[test]
fn rust_binding_rules_follow_each_pattern_and_the_lexical_definition() {
    let ir = nodex_lang_rust::lower_source("test.rs", r#"
        fn run(pair: (i32, i32), mut param: i32) {
            let (mut a, b) = pair;
            let r = &mut a;
            *r += 1;
            touch(r);
            *r += 1;
            let b = if param > 0 { param += 1; b + 1 } else { b - 1 };
            let delayed: i32;
            delayed = b;
            let mut direct = 0;
            let ref mut borrowed_pattern = direct;
            { let separate = 1; }
            { let separate = 2; }
        }
        fn touch(_: &mut i32) {}
    "#);
    let graph = ir.flows.iter().find(|g| g.name == "run").unwrap();
    let pattern = statement(graph, "let (mut a");
    assert!(output(pattern, "a").binding.as_ref().unwrap().reassignable);
    assert!(!output(pattern, "b").binding.as_ref().unwrap().reassignable);
    let borrow = statement(graph, "let r");
    assert!(!output(borrow, "r").binding.as_ref().unwrap().reassignable);
    assert!(graph.edges.iter().any(|edge| edge.from == pattern.id && edge.to == borrow.id && edge.access == Some(Access::BorrowMut)));
    let touch = statement(graph, "touch(r)");
    assert!(graph.edges.iter().any(|edge| edge.from == borrow.id && edge.to == touch.id && edge.access == Some(Access::ByValue)));
    let shadow = statement(graph, "let b =");
    let previous = output(shadow, "b").binding.as_ref().unwrap().shadows.as_ref().unwrap();
    assert_eq!((&previous.node, previous.pin.as_str()), (&pattern.id, "var:b"));
    for arm in ["b + 1", "b - 1"] {
        let arm = statement(graph, arm);
        assert!(graph.edges.iter().any(|edge| edge.to == arm.id && edge.from == pattern.id && edge.from_pin == "var:b"));
    }
    assert!(!output(statement(graph, "let delayed"), "delayed").binding.as_ref().unwrap().has_initializer);
    assert!(output(statement(graph, "let separate = 2"), "separate").binding.as_ref().unwrap().shadows.is_none());
    assert!(output(&graph.nodes[0], "param").binding.as_ref().unwrap().reassignable);
    assert!(output(statement(graph, "let mut direct"), "direct").binding.as_ref().unwrap().reassignable);
    assert!(!output(statement(graph, "let ref mut"), "borrowed_pattern").binding.as_ref().unwrap().reassignable);
}

#[test]
fn rust_mut_applies_to_the_name_captured_by_at_not_its_subpattern() {
    let ir = nodex_lang_rust::lower_source("test.rs", r#"
        fn run() -> i32 {
            let mut whole @ (first, second) = (1, 2);
            whole = (3, 4);
            let (mut part @ (left, right), rest) = ((1, 2), 3);
            part = (4, 5);
            let Whole @ (a, b) = (5, 6);
            Whole.0 + a + b
        }
    "#);
    let graph = ir.flows.iter().find(|g| g.name == "run").unwrap();
    for (code, mutable, fixed) in [
        ("let mut whole @", "whole", &["first", "second"][..]),
        ("let (mut part @", "part", &["left", "right", "rest"][..]),
    ] {
        let declaration = statement(graph, code);
        assert!(output(declaration, mutable).binding.as_ref().unwrap().reassignable);
        for name in fixed {
            assert!(!output(declaration, name).binding.as_ref().unwrap().reassignable);
        }
    }
    let declaration = statement(graph, "let Whole @");
    assert!(!output(declaration, "Whole").binding.as_ref().unwrap().reassignable);
    let use_site = statement(graph, "Whole.0 +");
    assert!(graph.edges.iter().any(|edge| edge.from == declaration.id && edge.from_pin == "var:Whole" && edge.to == use_site.id));
}

#[test]
fn typescript_defaults_and_rest_attach_to_the_binding_that_receives_them() {
    let ir = nodex_lang_typescript::lower_source("test.ts", r#"
        function run(fallback = 5) {
            const { value = fallback, ...rest } = input;
            let object = { value, ...rest };
            if (object) { var hoisted = 0; }
            return object.value || hoisted;
        }
    "#);
    let graph = ir.flows.iter().find(|g| g.name == "run").unwrap();
    let pattern = statement(graph, "const {");
    assert!(pattern.flavors.iter().any(|f| f == "binding-default:value"));
    assert!(pattern.flavors.iter().any(|f| f == "rest-properties:rest"));
    assert!(!output(pattern, "rest").binding.as_ref().unwrap().reassignable);
    assert!(graph.nodes[0].flavors.iter().any(|f| f == "parameter-default:fallback"));
    assert!(output(statement(graph, "let object"), "object").binding.as_ref().unwrap().reassignable);
    assert_eq!(output(statement(graph, "var hoisted"), "hoisted").binding.as_ref().unwrap().form, BindingForm::Var);
    assert!(graph.edges.iter().any(|e| e.from == statement(graph, "var hoisted").id &&
        e.to == statement(graph, "return").id && e.to_pin == "use:hoisted"));
    assert!(statement(graph, "return").expr.as_ref().unwrap().flavors.iter().any(|f| f == "falsy-fallback"));
}

#[test]
fn typescript_for_var_stays_visible_and_bare_redeclarations_preserve_values() {
    let ir = nodex_lang_typescript::lower_source("test.ts", r#"
        function run(value: number) {
            var value: number;
            for (var i = 0; i < 3; i++) { console.log(i); }
            for (let i = 0; i < 1; i++) { console.log(i); }
            for (var i: number; false;) {}
            return i + value;
        }
    "#);
    let graph = ir.flows.iter().find(|g| g.name == "run").unwrap();
    let entry = &graph.nodes[0];
    let var_loop = statement(graph, "for (var i = 0");
    let let_loop = statement(graph, "for (let i");
    let result = statement(graph, "return");
    let origin = |target: &FlowNode, name: &str| graph.edges.iter()
        .find(|edge| edge.to == target.id && edge.to_pin == format!("use:{name}")).unwrap().from.as_str();
    assert_eq!(output(var_loop, "i").binding.as_ref().unwrap().form, BindingForm::Var);
    assert_eq!(output(let_loop, "i").binding.as_ref().unwrap().form, BindingForm::Let);
    assert_eq!(origin(result, "i"), var_loop.id);
    assert_eq!(origin(result, "value"), entry.id);
    assert!(statement(graph, "var value: number;").writes.is_empty());
    assert!(statement(graph, "for (var i: number").writes.is_empty());
    let mut calls = graph.nodes.iter().filter(|node| node.code == "console.log(i);");
    assert_eq!(origin(calls.next().unwrap(), "i"), var_loop.id);
    assert_eq!(origin(calls.next().unwrap(), "i"), let_loop.id);
}

#[test]
fn typescript_conditional_for_redeclaration_updates_the_existing_var() {
    let ir = nodex_lang_typescript::lower_source("test.ts", r#"
        function run(flag: boolean) {
            var i = 7;
            if (flag) { for (var i = 0; i < 2; i++) {} }
            return i;
        }
    "#);
    let graph = ir.flows.iter().find(|g| g.name == "run").unwrap();
    let definition = statement(graph, "var i = 7");
    let branch = statement(graph, "if (flag)");
    let loop_node = statement(graph, "for (var i");
    let result = statement(graph, "return i");
    assert!(loop_node.writes.iter().any(|name| name == "i"));
    assert!(loop_node.outputs.iter().any(|pin| pin.id == "write:i"));
    assert!(!loop_node.outputs.iter().any(|pin| pin.id == "var:i"));
    let origins: Vec<_> = graph.edges.iter().filter(|edge| edge.to == result.id && edge.to_pin == "use:i").collect();
    assert_eq!(origins.len(), 1);
    assert_eq!(origins[0].from, definition.id);
    assert!(graph.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Exec &&
        edge.from == branch.id && edge.from_pin == "true" && edge.to == loop_node.id));
    assert!(graph.edges.iter().any(|edge| edge.kind == FlowEdgeKind::Exec &&
        edge.from == branch.id && edge.from_pin == "out" && edge.to == result.id));
}

#[test]
fn csharp_separates_inferred_locals_constants_and_runtime_paths() {
    let ir = nodex_lang_csharp::lower_source("Test.cs", r#"
        class Test {
            IEnumerable<int> Run() {
                const int limit = 1;
                var number = limit;
                ref readonly int alias = ref number;
                try { number = checked(number + 1); }
                catch (Exception) when (number > 1) { number = 0; }
                yield return number;
            }
        }
    "#);
    let graph = ir.flows.iter().find(|g| g.name == "Run").unwrap();
    let limit = output(statement(graph, "const int"), "limit").binding.as_ref().unwrap();
    assert_eq!(limit.form, BindingForm::Const);
    assert!(!limit.reassignable);
    let number = output(statement(graph, "var number"), "number").binding.as_ref().unwrap();
    assert_eq!(number.form, BindingForm::Inferred);
    assert!(number.reassignable);
    let alias = statement(graph, "ref readonly int alias");
    let alias_binding = output(alias, "alias").binding.as_ref().unwrap();
    assert_eq!(alias_binding.form, BindingForm::RefReadonly);
    assert!(alias_binding.reassignable, "the alias can point to another variable");
    assert!(!alias.flavors.iter().any(|f| f == "ref"));
    let attempt = statement(graph, "try");
    assert!(attempt.flavors.iter().any(|f| f == "catch-filter:0"));
    assert!(attempt.outputs.iter().any(|p| p.id == "catch:0"));
    assert!(statement(graph, "yield return").flavors.iter().any(|f| f == "yield-item"));
}


fn syntax<'a>(ir: &'a nodex_ir::FileIr) -> Vec<&'a nodex_ir::SyntaxSite> {
    fn expr<'a>(e: &'a nodex_ir::Expr, sites: &mut Vec<&'a nodex_ir::SyntaxSite>) {
        sites.extend(e.syntax_sites.iter().flatten());
        for child in &e.children { expr(child, sites); }
    }
    let mut sites = Vec::new();
    for node in ir.flows.iter().flat_map(|f| &f.nodes) {
        sites.extend(node.syntax_sites.iter().flatten());
        if let Some(e) = &node.expr { expr(e, &mut sites); }
    }
    sites
}

#[test]
fn syntax_sites_keep_occurrences_and_operator_context() {
    let csharp = r#"class C { int F(int[] a, int x) { var emoji = "🚀^"; return (a[^1] ^ x) + a[^2]; } }"#;
    let cs = nodex_lang_csharp::lower_source("C.cs", csharp);
    let mut caret: Vec<_> = syntax(&cs).into_iter().filter(|s| s.key == "index-from-end")
        .map(|s| s.span.start_byte as usize).collect();
    caret.sort_unstable();
    assert_eq!(caret, [csharp.find("^1").unwrap(), csharp.find("^2").unwrap()]);
    let ts = r#"function f(a: number[], ...items: number[]) { const [x, ...rest] = a; return [0, ...rest]; }"#;
    let ts = nodex_lang_typescript::lower_source("src/f.ts", ts);
    let sites = syntax(&ts);
    for (key, pin) in [("rest-arguments", "var:items"), ("rest-items", "var:rest")] {
        assert!(sites.iter().any(|s| s.key == key && s.pin.as_deref() == Some(pin)), "{sites:?}");
    }
    assert!(sites.iter().any(|s| s.key == "spread-items"));
}

#[test]
fn rust_syntax_anchors_cover_operators_even_when_lowering_unwraps_them() {
    let src = r#"fn f(x: &mut i32, y: Option<i32>) -> Option<i32> { let r = &mut *x; let a = y?; match a { n if n > 0 => Some(*r), _ => None } }"#;
    let ir = nodex_lang_rust::lower_source("src/lib.rs", src);
    let sites = syntax(&ir);
    for (key, token) in [("borrow-mut", "&mut"), ("deref", "*"), ("try", "?"), ("match-guard", "if")] {
        assert!(sites.iter().any(|s| s.key == key && &src[s.span.start_byte as usize..s.span.end_byte as usize] == token), "{key}: {sites:?}");
    }
}

#[test]
fn rust_match_guards_stay_on_expression_arms_or_statement_pins() {
    use nodex_ir::Concept;

    let source = r#"
        fn f(pair: (i32, i32)) -> i32 {
            let next = match pair {
                (n, _) | (_, n) if n > 0 => n,
                (n, _) if n < 0 => -n,
                _ => 0,
            };
            match next { n if n > 0 => n, _ => 0 }
        }
    "#;
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let graph = ir.flows.iter().find(|graph| graph.name == "f").unwrap();
    let expression = statement(graph, "let next").expr.as_ref().unwrap();
    assert_eq!(expression.concept, Concept::Match);
    for (index, code) in [(1, "if n > 0"), (2, "if n < 0")] {
        let arm = &expression.children[index];
        let sites = arm.syntax_sites.as_deref().unwrap();
        let guard = sites.iter().find(|site| site.key == "match-guard").unwrap();
        assert_eq!(guard.pin, None);
        assert_eq!(guard.span.start_byte as usize, source.find(code).unwrap());
        assert_eq!(&source[guard.span.start_byte as usize..guard.span.end_byte as usize], "if");
    }
    let statement = statement(graph, "match next");
    let guard = statement.syntax_sites.iter().flatten().find(|site| site.key == "match-guard").unwrap();
    assert_eq!(guard.pin.as_deref(), Some("case:0"));
    assert_eq!(guard.span.start_byte as usize, source.rfind("if n > 0").unwrap());
    assert!(statement.outputs.iter().any(|pin| pin.id == "case:0"));
    assert_eq!(syntax(&ir).iter().filter(|site| site.key == "match-guard").count(), 3);
}


#[test]
fn syntax_only_value_shape_is_independent_of_reference_passing() {
    use nodex_ir::{PassingMode, ValueKind, ValueQualifier};
    let csharp = nodex_lang_csharp::lower_source("C.cs", "unsafe class C { void F(int i, ref int n, string s, int* p) {} }");
    let entry = &csharp.flows.iter().find(|f| f.name == "F").unwrap().nodes[0];
    for (name, kind, passing) in [("i", ValueKind::Value, PassingMode::ByValue),
        ("n", ValueKind::Reference, PassingMode::Ref), ("s", ValueKind::Reference, PassingMode::ByValue),
        ("p", ValueKind::Pointer, PassingMode::ByValue)] {
        let info = output(entry, name).value_info.unwrap_or_else(|| panic!("missing shape for {name}: {entry:?}"));
        assert_eq!((info.kind, info.passing), (kind, Some(passing)));
    }

    let rust = nodex_lang_rust::lower_source("src/lib.rs", "fn f() { let mut n = 1; let r = &mut n; let p = &raw const n; }");
    let flow = &rust.flows[0];
    assert_eq!(output(statement(flow, "let mut n"), "n").value_info.unwrap().kind, ValueKind::Value);
    assert_eq!(output(statement(flow, "let r"), "r").value_info.unwrap().qualifier, Some(ValueQualifier::ExclusiveBorrow));
    assert_eq!(output(statement(flow, "let p"), "p").value_info.unwrap().kind, ValueKind::Pointer);

    let js = nodex_lang_typescript::lower_source("f.js", "function f() { const s = 'x'; const o = {}; }");
    let flow = &js.flows.iter().find(|f| f.name == "f").unwrap();
    assert_eq!(output(statement(flow, "const s"), "s").value_info.unwrap().kind, ValueKind::Value);
    assert_eq!(output(statement(flow, "const o"), "o").value_info.unwrap().kind, ValueKind::Reference);
}


#[test]
fn rust_borrows_follow_places_reborrows_reassignments_and_branch_joins() {
    use nodex_ir::borrow::BorrowRoot;
    let source = r#"
        struct Data { name: String }
        fn f(choose: bool) {
            let a = Data { name: String::new() };
            let b = Data { name: String::new() };
            let mut r = &a.name;
            let before = &*r;
            let slot = &r;
            if choose { r = &b.name; }
            let after = &*r;
            { let a = Data { name: String::new() }; let shadow = &a.name; }
        }
    "#;
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let model = &ir.borrow_models[&flow.symbol];
    let paths = |name: &str| {
        let at = source.find(&format!("{name} =")).unwrap() as u32;
        &model.uses.iter().find(|site| site.span.start_byte == at).unwrap().paths
    };
    let roots = |name| paths(name).iter().filter_map(|p| match &p.root {
        BorrowRoot::Local(binding) => Some(binding.name.as_str()),
        _ => None,
    }).collect::<Vec<_>>();
    assert_eq!(roots("before"), ["a"]);
    assert_eq!(paths("before")[0].projection, ".name");
    assert_eq!(paths("before")[0].through.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["r", "before"]);
    assert_eq!(roots("slot"), ["r"]);
    let mut after = roots("after");
    after.sort_unstable();
    assert_eq!(after, ["a", "b"]);
    assert!(matches!(&paths("shadow")[0].root, BorrowRoot::Local(binding)
        if binding.span.start_byte as usize == source.rfind("a = Data").unwrap()));
}


#[test]
fn rust_borrows_merge_loop_paths_and_forget_references_written_through_aliases() {
    use nodex_ir::borrow::BorrowRoot;
    let source = r#"
        fn modify<'a>(p: &mut &'a i32, next: &'a i32) { *p = next; }
        fn f(mut repeat: bool) {
            let a = 1;
            let b = 2;
            let mut r = &a;
            while repeat { r = &b; repeat = false; }
            let after_loop = r;
            modify(&mut r, &b);
            let after_alias = r;
        }
    "#;
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let model = &ir.borrow_models[&flow.symbol];
    let paths = |name: &str| {
        let at = source.find(&format!("{name} =")).unwrap() as u32;
        &model.uses.iter().find(|site| site.span.start_byte == at).unwrap().paths
    };
    let mut owners = paths("after_loop").iter().filter_map(|path| match &path.root {
        BorrowRoot::Local(binding) => Some(binding.name.as_str()),
        _ => None,
    }).collect::<Vec<_>>();
    owners.sort_unstable();
    assert_eq!(owners, ["a", "b"]);
    assert!(paths("after_alias").iter().all(|path| matches!(path.root, BorrowRoot::Unknown)));
}

#[test]
fn rust_borrows_follow_short_circuits_and_labeled_exits() {
    use nodex_ir::borrow::BorrowRoot;
    let source = r#"
        fn origins(flag: bool) {
            let a = 1;
            let b = 2;
            let mut r = &a;
            let _ = flag && { r = &b; true };
            let after_and = r;
            r = &a;
            let _ = flag || { r = &b; false };
            let after_or = r;
            r = &a;
            'outer: loop {
                loop { r = &b; break 'outer; }
                r = &a;
                break;
            }
            let after_break = r;
            r = &a;
            'outer: while *r == 1 {
                loop { r = &b; continue 'outer; }
                r = &a;
            }
            let after_continue = r;
            let from_loop = loop { break &b; };
            let from_block = 'block: {
                loop { break 'block &b; }
            };
            let mut writable = 0;
            let from_mutable_break = loop { break &mut writable; };
        }
        fn returned(x: &i32) -> &i32 {
            loop { break x; }
        }
    "#;
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let model = &ir.borrow_models[&ir.flows[0].symbol];
    for (name, expected) in [
        ("after_and", &["a", "b"][..]),
        ("after_or", &["a", "b"][..]),
        ("after_break", &["b"][..]),
        ("after_continue", &["a", "b"][..]),
        ("from_loop", &["b"][..]),
        ("from_block", &["b"][..]),
        ("from_mutable_break", &["writable"][..]),
    ] {
        let at = source.find(&format!("{name} =")).unwrap() as u32;
        let paths = &model.uses.iter().find(|site| site.span.start_byte == at)
            .unwrap_or_else(|| panic!("no paths for {name}")).paths;
        let owners: std::collections::BTreeSet<_> = paths.iter().map(|path| match &path.root {
            BorrowRoot::Local(binding) => binding.name.as_str(),
            root => panic!("{name} has unexpected root {root:?}"),
        }).collect();
        assert_eq!(owners.into_iter().collect::<Vec<_>>(), expected, "{name}");
        if name == "from_mutable_break" {
            assert!(paths.iter().all(|path| path.mutable));
        }
    }
    let returned = ir.flows.iter().find(|flow| flow.name == "returned").unwrap();
    let paths = &ir.borrow_models[&returned.symbol].returns;
    assert!(matches!(paths.as_slice(), [path] if matches!(path.root, BorrowRoot::Parameter(0))));
    for name in ["from_loop", "from_block", "from_mutable_break"] {
        let pin = ir.flows[0].nodes.iter().flat_map(|node| &node.outputs)
            .find(|pin| pin.id == format!("var:{name}")).unwrap();
        assert_eq!(pin.value_info.unwrap().kind, nodex_ir::ValueKind::Reference, "{name}");
    }
}

#[test]
fn rust_only_attributes_field_borrows_to_proven_receivers() {
    use nodex_ir::borrow::BorrowRoot;
    let source = r#"
        struct Data { field: i32 }
        struct Wrap {}
        struct Nested { data: Data }
        static GLOBAL: Data = Data { field: 9 };
        impl std::ops::Deref for Wrap {
            type Target = Data;
            fn deref(&self) -> &Data { &GLOBAL }
        }
        fn origins(parameter: Data, wrapper: Wrap, nested: Nested) {
            let local = Data { field: 1 };
            let reference = &local;
            let literal = Wrap {};
            let direct = &local.field;
            let through_reference = &reference.field;
            let through_parameter = &parameter.field;
            let through_nested = &nested.data.field;
            let implicit_deref = &literal.field;
            let typed_deref = &wrapper.field;
        }
    "#;
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "origins").unwrap();
    let model = &ir.borrow_models[&flow.symbol];
    for (name, expected) in [
        ("direct", Some("local")),
        ("through_reference", Some("local")),
        ("through_parameter", Some("parameter")),
        ("through_nested", Some("nested")),
        ("implicit_deref", None),
        ("typed_deref", None),
    ] {
        let at = source.find(&format!("{name} =")).unwrap() as u32;
        let paths = &model.uses.iter().find(|site| site.span.start_byte == at).unwrap().paths;
        assert_eq!(paths.len(), 1);
        match (&paths[0].root, expected) {
            (BorrowRoot::Local(binding), Some(owner)) => {
                assert_eq!(binding.name, owner);
                let projection = if name == "through_nested" { ".data.field" } else { ".field" };
                assert_eq!(paths[0].projection, projection);
            }
            (BorrowRoot::Unknown, None) => {}
            result => panic!("{name} has unexpected origin {result:?}"),
        }
    }
}

#[test]
fn csharp_readonly_details_match_the_declaration() {
    let source = r#"
        public readonly struct Frozen {
            private readonly int count;
        }
        public struct Counter {
            private int count;
            public readonly int Read() => count;
            public int Total { readonly get; set; }
            public int Computed { readonly get => count; set { count = value; } }
        }
    "#;
    let ir = nodex_lang_csharp::lower_source("Counters.cs", source);
    for (name, key) in [
        ("Frozen", "readonly-struct"),
        ("count", "readonly-field"),
        ("Read", "readonly-member"),
        ("Total", "readonly-member"),
        ("Computed.get", "readonly-member"),
    ] {
        let sites: Vec<_> = ir.symbols.iter().filter(|symbol| symbol.name == name)
            .flat_map(|symbol| symbol.syntax_sites.iter().flatten()).collect();
        assert_eq!(sites.len(), 1, "{name}");
        let site = sites[0];
        assert_eq!(site.key, key);
        assert_eq!(&source[site.span.start_byte as usize..site.span.end_byte as usize], "readonly");
    }
    let computed = ir.symbols.iter().find(|symbol| symbol.name == "Computed").unwrap();
    assert!(computed.syntax_sites.iter().flatten().all(|site| site.key != "readonly-member"));
}

#[test]
fn syntax_value_shapes_respect_grouping_and_outer_arrays() {
    use nodex_ir::ValueKind;
    let rust = nodex_lang_rust::lower_source("src/lib.rs", "fn f(r: ((&i32)), p: (*const i32), tuple: (&i32,)) {}");
    let csharp = nodex_lang_csharp::lower_source("Pointers.cs", "unsafe class C { void F(delegate*<void>[] callbacks, delegate*<void> callback) {} }");
    for (ir, expected) in [
        (rust, &[("r", ValueKind::Reference), ("p", ValueKind::Pointer), ("tuple", ValueKind::Value)][..]),
        (csharp, &[("callbacks", ValueKind::Reference), ("callback", ValueKind::Pointer)][..]),
    ] {
        for (name, kind) in expected {
            let pin = ir.flows[0].nodes[0].outputs.iter().find(|pin| pin.id == format!("var:{name}")).unwrap();
            assert_eq!(pin.value_info.unwrap().kind, *kind, "{name}");
        }
    }
}

#[test]
fn rust_control_flow_values_preserve_raw_pointer_coercion() {
    use nodex_ir::borrow::BorrowRoot;
    use nodex_ir::{ValueKind, ValueQualifier};
    let source = r#"
        fn pointer(flag: bool) {
            let a = 1;
            let b = 2;
            let raw = loop {
                if flag { break &raw const b; }
                break &a;
            };
            let _: *const i32 = raw;
            let cast = loop { break &b as *const i32; };
            let _: *const i32 = cast;
            let existing: *const i32 = &a;
            let copied = loop { break existing; };
            let _: *const i32 = copied;
            let borrowed = loop { break &a; };
            let _: &i32 = borrowed;
        }
    "#;
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let flow = &ir.flows[0];
    let model = &ir.borrow_models[&flow.symbol];
    for name in ["raw", "cast", "copied"] {
        let pin = flow.nodes.iter().flat_map(|node| &node.outputs)
            .find(|pin| pin.id == format!("var:{name}")).unwrap();
        let info = pin.value_info.unwrap();
        assert_eq!(info.kind, ValueKind::Pointer, "{name}");
        assert_eq!(info.qualifier, Some(ValueQualifier::Raw), "{name}");
        let declaration = pin.declaration.as_ref().unwrap();
        let site = model.uses.iter().find(|site| &site.span == declaration).unwrap();
        assert!(matches!(site.paths.as_slice(), [path] if path.root == BorrowRoot::RawPointer));
    }
    let borrowed = flow.nodes.iter().flat_map(|node| &node.outputs)
        .find(|pin| pin.id == "var:borrowed").unwrap();
    assert_eq!(borrowed.value_info.unwrap().kind, ValueKind::Reference);
}

#[test]
fn rust_borrows_keep_short_circuit_outcomes_separate() {
    use nodex_ir::borrow::BorrowRoot;
    let source = r#"
        fn branch(flag: bool) {
            let a = 1;
            let b = 2;
            let mut r = &a;
            if flag && { r = &b; true /* keep the tail value */ } {
                let after_and = r;
            } else {
                let skipped_and = r;
            }
            r = &a;
            if !(flag || { r = &b; false }) {
                let after_or = r;
            } else {
                let skipped_or = r;
            }
            r = &a;
            while flag && { r = &b; true } {
                let inside_loop = r;
                break;
            }
        }
    "#;
    let ir = nodex_lang_rust::lower_source("src/lib.rs", source);
    let model = &ir.borrow_models[&ir.flows[0].symbol];
    for (name, owner) in [
        ("after_and", "b"),
        ("skipped_and", "a"),
        ("after_or", "b"),
        ("skipped_or", "a"),
        ("inside_loop", "b"),
    ] {
        let at = source.find(&format!("{name} =")).unwrap() as u32;
        let site = model.uses.iter().find(|site| site.span.start_byte == at).unwrap();
        assert!(!site.paths.is_empty(), "{name}");
        assert!(site.paths.iter().all(|path| matches!(
            &path.root, BorrowRoot::Local(binding) if binding.name == owner
        )), "{name}: {:?}", site.paths);
    }
}
