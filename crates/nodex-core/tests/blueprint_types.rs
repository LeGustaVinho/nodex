use std::path::Path;
use std::sync::Arc;

use nodex_core::Project;
use nodex_ir::types::{CollectionKind, CollectionRole, TypeInfo, TypeOrigin, TypeShape};
use nodex_ir::{FlowGraph, FlowNode, Language};
use nodex_lang_api::LanguageFrontend;
use nodex_lang_csharp::CSharpFrontend;
use nodex_lang_rust::RustFrontend;
use nodex_lang_typescript::TypeScriptFrontend;

fn scalar(name: &str) -> TypeShape {
    TypeShape::scalar(name)
}

fn collection(family: CollectionKind, types: impl IntoIterator<Item = TypeShape>) -> TypeShape {
    TypeShape::collection(family, types.into_iter().collect())
}

fn statement<'a>(flow: &'a FlowGraph, code: &str) -> &'a FlowNode {
    flow.nodes
        .iter()
        .find(|node| node.code.starts_with(code))
        .unwrap_or_else(|| panic!("no statement {code}"))
}

fn binding<'a>(flow: &'a FlowGraph, name: &str) -> &'a TypeInfo {
    let pin = flow
        .nodes
        .iter()
        .flat_map(|node| &node.outputs)
        .find(|pin| pin.id == format!("var:{name}"))
        .unwrap_or_else(|| panic!("no binding {name} in {}", flow.name));
    pin.type_info
        .as_ref()
        .unwrap_or_else(|| panic!("no type for {name} in {}: {pin:?}", flow.name))
}

fn pin_shape<'a>(node: &'a FlowNode, id: &str) -> Option<&'a TypeShape> {
    node.inputs
        .iter()
        .chain(&node.outputs)
        .find(|pin| pin.id == id)?
        .type_info
        .as_ref()
        .map(|info| &info.shape)
}

#[test]
fn syntax_types_follow_the_lexical_binding_and_reaching_assignments() {
    let ir = nodex_lang_typescript::lower_source(
        "syntax.js",
        r#"
        function example(flag) {
            const items = [1, 2];
            const empty = [];
            const [count, text] = [1, "word"];
            const { status: good, pair: [, deep] } = { status: true, pair: [0, "nested"] };
            let x = 1;
            first(x);
            x = "string";
            second(x);
            const copy = x;
            {
                const x = true;
                shadow(x);
            }
            outer(x);
            if (flag) { x = 2; }
            else { x = false; }
            joined(x);
            while (flag) { x = 42; break; }
            afterLoop(x);
            x = getUnknown();
            unknown(x);
        }
    "#,
    );
    let flow = ir.flows.iter().find(|flow| flow.name == "example").unwrap();
    assert_eq!(
        binding(flow, "items").shape,
        collection(CollectionKind::Array, [scalar("Number")])
    );
    assert_eq!(
        binding(flow, "empty").shape,
        collection(CollectionKind::Array, [TypeShape::Unknown])
    );
    assert_eq!(binding(flow, "copy").shape, scalar("String"));
    for (name, shape) in [
        ("count", "Number"),
        ("text", "String"),
        ("good", "Bool"),
        ("deep", "String"),
    ] {
        assert_eq!(binding(flow, name).shape, scalar(shape), "{name}");
    }
    let read = |code| {
        statement(flow, code)
            .inputs
            .iter()
            .find(|pin| pin.id == "use:x")
            .unwrap()
            .type_info
            .as_ref()
            .map(|info| &info.shape)
    };
    for (code, shape) in [
        ("first", scalar("Number")),
        ("second", scalar("String")),
        ("shadow", scalar("Bool")),
        ("outer", scalar("String")),
    ] {
        assert_eq!(read(code), Some(&shape), "{code}");
    }
    let Some(TypeShape::Union { items }) = read("joined") else {
        panic!("expected a union: {:?}", read("joined"))
    };
    assert_eq!(items.len(), 2);
    assert!(items.contains(&scalar("Number")) && items.contains(&scalar("Bool")));
    assert!(read("afterLoop").is_none_or(|shape| *shape == TypeShape::Unknown));
    assert!(read("unknown").is_none_or(|shape| *shape == TypeShape::Unknown));
}

#[test]
fn syntax_types_apply_nested_writes_in_evaluation_order() {
    let ir = nodex_lang_typescript::lower_source(
        "syntax.js",
        r#"
        function example(flag, call, obj) {
            let x = 1;
            nested(x, (x = "text"), x);
            afterCall(x);
            if (x = true) { inside(x); }
            afterCondition(x);
            flag && (x = "text");
            afterShortCircuit(x);
            x = 1;
            call?.(x = "text");
            afterOptional(x);
            x = 1;
            obj?.nested.consume(x = "text");
            afterChain(x);
            x = 1;
            obj?.nested[(x = "text")];
            afterIndex(x);
            x = 1;
            obj?.consume(x = "text").then(probe(x));
            afterCallChain(x);
            x = 1;
            (obj?.nested).consume(x = "text");
            afterParentheses(x);
        }
    "#,
    );
    let flow = ir.flows.iter().find(|flow| flow.name == "example").unwrap();
    let nested = statement(flow, "nested(").expr.as_ref().unwrap();
    assert_eq!(
        nested.children[0].type_info.as_ref().unwrap().shape,
        scalar("Number")
    );
    assert_eq!(
        nested.children[2].type_info.as_ref().unwrap().shape,
        scalar("String")
    );
    for (code, ty) in [
        ("afterCall", "String"),
        ("inside", "Bool"),
        ("afterCondition", "Bool"),
        ("obj?.consume", "String"),
        ("afterParentheses", "String"),
    ] {
        assert_eq!(
            pin_shape(statement(flow, code), "use:x"),
            Some(&scalar(ty)),
            "{code}"
        );
    }
    for (code, initial) in [
        ("afterShortCircuit", "Bool"),
        ("afterOptional", "Number"),
        ("afterChain", "Number"),
        ("afterIndex", "Number"),
        ("afterCallChain", "Number"),
    ] {
        let shape = pin_shape(statement(flow, code), "use:x");
        assert!(
            matches!(shape, Some(TypeShape::Union { items })
            if items.len() == 2 && items.contains(&scalar(initial)) && items.contains(&scalar("String"))),
            "{code}: {shape:?}"
        );
    }
}

#[test]
fn syntax_types_follow_writes_in_immediate_calls_and_case_tests() {
    let ir = nodex_lang_typescript::lower_source(
        "syntax.js",
        r#"
        function example(flag) {
            let x = 1;
            (() => { let x = false; x = "inner"; })();
            shadowed(x);
            ((x) => { x = "parameter"; })(0);
            parameter(x);
            (() => { x = "text"; })();
            afterImmediate(x);
            x = 1;
            switch (flag) { case (x = "label"): break; default: break; }
            afterCase(x);
            x = 1;
            switch (flag) { case 0: break; case (x = "label"): break; }
            afterLaterCase(x);
            (() => { if (flag) x = true; })();
            afterUncertain(x);
        }
    "#,
    );
    let flow = ir.flows.iter().find(|flow| flow.name == "example").unwrap();
    for (code, expected) in [
        ("shadowed", scalar("Number")),
        ("parameter", scalar("Number")),
        ("afterImmediate", scalar("String")),
        ("afterCase", scalar("String")),
        ("afterUncertain", TypeShape::Unknown),
    ] {
        assert_eq!(
            pin_shape(statement(flow, code), "use:x"),
            Some(&expected),
            "{code}"
        );
    }
    let shape = pin_shape(statement(flow, "afterLaterCase"), "use:x");
    assert!(
        matches!(shape, Some(TypeShape::Union { items })
        if items.len() == 2 && items.contains(&scalar("String")) && items.contains(&scalar("Number"))),
        "{shape:?}"
    );
}

#[test]
fn syntax_types_use_csharp_binding_and_conversion_evidence() {
    let ir = nodex_lang_csharp::lower_source(
        "Syntax.cs",
        r#"
        class Example {
            void Test(bool flag) {
                int count = 0;
                System.Console.WriteLine(count);
                Set(out count);
                var conditional = flag ? 1 : 2L;
                var items = new[] { 1, 2L };
                var jagged = new int[2][];
                var matrix = new int[2, 3][];
                var (number, text) = (1L, "text");
            }
            void Set(out int value) => value = 1;
        }
    "#,
    );
    let flow = ir.flows.iter().find(|flow| flow.name == "Test").unwrap();
    assert_eq!(
        pin_shape(statement(flow, "System.Console"), "use:count"),
        Some(&scalar("Int32"))
    );
    assert_eq!(
        pin_shape(statement(flow, "Set("), "write:count"),
        Some(&scalar("Int32"))
    );
    assert_eq!(binding(flow, "conditional").shape, scalar("Int64"));
    assert_eq!(
        binding(flow, "items").shape,
        collection(CollectionKind::Array, [scalar("Int64")])
    );
    assert_eq!(binding(flow, "number").shape, scalar("Int64"));
    assert_eq!(binding(flow, "text").shape, scalar("String"));
    let array = collection(CollectionKind::Array, [scalar("Int32")]);
    assert_eq!(
        binding(flow, "jagged").shape,
        collection(CollectionKind::Array, [array.clone()])
    );
    assert_eq!(
        binding(flow, "matrix").shape,
        TypeShape::Collection {
            family: CollectionKind::Array,
            arguments: vec![array],
            rank: Some(2),
            length: None,
        }
    );
}

#[test]
fn rust_syntax_counts_elements_and_distinguishes_c_strings() {
    let ir = nodex_lang_rust::lower_source(
        "syntax.rs",
        r#"fn example() {
            let items = [1u64, /* note */ 2u64];
            let tuple = (1_i32, /* note */ 2_u64);
            let c_string = c"text";
        }"#,
    );
    let flow = ir.flows.iter().find(|flow| flow.name == "example").unwrap();
    assert_eq!(
        binding(flow, "items").shape,
        TypeShape::Collection {
            family: CollectionKind::Array,
            arguments: vec![scalar("UInt64")],
            rank: None,
            length: Some("2".into()),
        }
    );
    assert_eq!(
        binding(flow, "tuple").shape,
        TypeShape::Tuple {
            items: vec![scalar("Int32"), scalar("UInt64")]
        }
    );
    assert_eq!(binding(flow, "c_string").shape, c_string_shape());
}

fn c_string_shape() -> TypeShape {
    TypeShape::Indirect {
        pointer: false,
        target: Box::new(TypeShape::Named {
            name: "CStr".into(),
            arguments: Vec::new(),
        }),
    }
}

fn analyze(frontend: impl LanguageFrontend + 'static) -> Project {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rosetta/language-details");
    let mut project = Project::open(&root, vec![Arc::new(frontend)]).unwrap();
    let frontend = project.active_frontends().into_iter().next().unwrap();
    let facts = frontend
        .semantic(&root)
        .unwrap()
        .unwrap()
        .analyze()
        .unwrap();
    project.apply_semantic(frontend.id(), facts);
    project
}

fn flow(project: &mut Project, name: &str) -> FlowGraph {
    let id = project
        .flows()
        .find(|flow| flow.name == name)
        .unwrap()
        .symbol
        .clone();
    project.flow(&id).unwrap()
}

#[test]
#[ignore = "needs Roslyn and the .NET SDK"]
fn roslyn_preserves_identities_dimensions_and_nested_member_arguments() {
    use CollectionKind::*;
    let mut project = analyze(CSharpFrontend);
    let flow = flow(&mut project, "Collections");
    assert_eq!(
        binding(&flow, "items").shape,
        collection(List, [scalar("Int32")])
    );
    assert_eq!(binding(&flow, "items").origin, TypeOrigin::Compiler);
    assert_eq!(
        binding(&flow, "own").shape,
        TypeShape::Named {
            name: "List".into(),
            arguments: vec![scalar("Int32")]
        }
    );
    assert_eq!(
        binding(&flow, "rectangle").shape,
        TypeShape::Collection {
            family: Array,
            arguments: vec![scalar("Float64")],
            rank: Some(2),
            length: None,
        }
    );
    assert_eq!(binding(&flow, "count").shape, scalar("Int64"));
    assert_eq!(binding(&flow, "title").shape, scalar("String"));
    assert_eq!(binding(&flow, "anonymous").shape, scalar("Object"));
    for name in ["captured", "loopUpdate"] {
        assert!(binding(&flow, name).role.is_none(), "{name}");
    }
    assert_eq!(
        pin_shape(statement(&flow, "Set("), "write:updated"),
        Some(&scalar("Int32"))
    );
    assert_eq!(
        binding(&flow, "fifo")
            .role
            .as_ref()
            .map(|evidence| evidence.role),
        Some(CollectionRole::Queue)
    );
    let graph = project.structure();
    let member = graph
        .symbols
        .iter()
        .find(|symbol| symbol.name == "Buckets")
        .unwrap();
    assert_eq!(
        member.type_info.as_ref().unwrap().shape,
        collection(
            Dictionary,
            [
                scalar("String"),
                collection(List, [collection(Array, [scalar("Int32")])])
            ]
        )
    );
}

#[test]
#[ignore = "needs the TypeScript sidecar"]
fn typescript_and_javascript_share_types_and_prove_roles_from_uses() {
    use CollectionKind::*;
    let mut project = analyze(TypeScriptFrontend);
    let typed = flow(&mut project, "typedCollections");
    assert_eq!(
        binding(&typed, "dictionary").shape,
        collection(Dictionary, [scalar("String"), scalar("Bool")])
    );
    assert_eq!(
        binding(&typed, "nested").shape,
        collection(
            Dictionary,
            [
                scalar("String"),
                collection(Array, [collection(Set, [scalar("Number")])])
            ]
        )
    );
    assert_eq!(
        binding(&typed, "own").shape,
        TypeShape::Named {
            name: "Queue".into(),
            arguments: vec![scalar("String")]
        }
    );
    assert_eq!(binding(&typed, "first").shape, scalar("Bool"));
    assert_eq!(
        binding(&typed, "second").shape,
        TypeShape::Tuple {
            items: vec![scalar("Number"), scalar("String")]
        }
    );
    let narrowed = statement(&typed, "const narrowed");
    assert_eq!(
        narrowed
            .inputs
            .iter()
            .find(|pin| pin.id == "use:value")
            .unwrap()
            .type_info
            .as_ref()
            .unwrap()
            .shape,
        scalar("String")
    );
    fn parameter(expr: &nodex_ir::Expr) -> Option<&TypeInfo> {
        expr.param_pins
            .iter()
            .flatten()
            .find(|pin| pin.label == "item")
            .and_then(|pin| pin.type_info.as_ref())
            .or_else(|| expr.children.iter().find_map(parameter))
    }
    let lambda_param =
        parameter(statement(&typed, "const converted").expr.as_ref().unwrap()).unwrap();
    assert_eq!(lambda_param.shape, scalar("Number"));
    let enums = flow(&mut project, "enumTypes");
    for (parameter, name) in [("phase", "Phase"), ("mode", "Mode")] {
        assert_eq!(
            binding(&enums, parameter).shape,
            TypeShape::Named {
                name: name.into(),
                arguments: vec![]
            }
        );
    }
    let arrow = flow(&mut project, "typedArrow");
    assert_eq!(binding(&arrow, "value").shape, scalar("Number"));
    let js = flow(&mut project, "scriptCollections");
    assert_eq!(
        binding(&js, "items").shape,
        collection(Array, [scalar("Number")])
    );
    for (name, expected) in [
        ("fifo", CollectionRole::Queue),
        ("lifo", CollectionRole::Stack),
    ] {
        let evidence = binding(&js, name).role.as_ref().expect(name);
        assert_eq!(evidence.role, expected);
        assert!(evidence.insert.start_line < evidence.remove.start_line);
    }
    for name in [
        "mixed",
        "escaped",
        "leaked",
        "loopUpdate",
        "captured",
        "changedLength",
    ] {
        assert!(binding(&js, name).role.is_none(), "{name}");
    }
    assert_eq!(
        binding(&js, "enqueue").role.as_ref().map(|role| role.role),
        Some(CollectionRole::Queue)
    );
    let changing = flow(&mut project, "scriptChangingTypes");
    for (code, shape) in [
        ("beforeWrite", scalar("Number")),
        ("afterWrite", scalar("String")),
        ("afterUnknown", TypeShape::Unknown),
    ] {
        assert_eq!(
            pin_shape(statement(&changing, code), "use:x"),
            Some(&shape),
            "{code}"
        );
    }
    let branches = pin_shape(statement(&changing, "afterBranch"), "use:x");
    assert!(matches!(branches, Some(TypeShape::Union { items })
        if items.len() == 2 && items.contains(&scalar("String")) && items.contains(&scalar("Bool"))));
}

#[test]
#[ignore = "needs rust-analyzer"]
fn rust_scip_types_resolve_standard_collections_and_deque_roles() {
    use CollectionKind::*;
    let mut project = analyze(RustFrontend);
    let collections = flow(&mut project, "rust_collections");
    let generic = flow(&mut project, "rust_generic_expression");
    let flow = &collections;
    assert_eq!(flow.language, Language::Rust);
    assert_eq!(
        binding(&flow, "items").shape,
        collection(List, [scalar("Int32")])
    );
    assert_eq!(
        binding(&flow, "own").shape,
        TypeShape::Named {
            name: "Vec".into(),
            arguments: vec![scalar("UInt64")]
        }
    );
    assert_eq!(
        binding(&flow, "nested").shape,
        collection(Deque, [collection(List, [scalar("Int32")])])
    );
    assert_eq!(
        binding(flow, "nested_vectors").shape,
        collection(List, [collection(List, [scalar("Int32")])])
    );
    assert_eq!(
        binding(flow, "custom_elements").shape,
        collection(
            List,
            [TypeShape::Named {
                name: "Vec".into(),
                arguments: vec![scalar("UInt64")],
            }]
        )
    );
    assert_eq!(
        binding(&flow, "copied_type").shape,
        TypeShape::Indirect {
            pointer: false,
            target: Box::new(binding(&flow, "nested").shape.clone()),
        }
    );
    let fifo = binding(&flow, "fifo");
    assert_eq!(fifo.shape, collection(Deque, [scalar("Int32")]));
    assert_eq!(
        fifo.role.as_ref().map(|role| role.role),
        Some(CollectionRole::Queue)
    );
    let defaulted = binding(flow, "defaulted");
    assert_eq!(defaulted.shape, collection(Deque, [scalar("Int32")]));
    assert_eq!(
        defaulted.role.as_ref().map(|role| role.role),
        Some(CollectionRole::Queue)
    );
    assert_eq!(
        binding(&flow, "lifo").role.as_ref().map(|role| role.role),
        Some(CollectionRole::Stack)
    );
    assert!(binding(&flow, "mixed").role.is_none());
    assert_eq!(
        binding(flow, "deconstructed").shape,
        collection(List, [scalar("Int32")])
    );
    assert_eq!(binding(flow, "marker").shape, scalar("UInt64"));

    let source = include_str!("../../../fixtures/rosetta/language-details/src/universal_types.rs");
    fn expression<'a>(
        expr: &'a nodex_ir::Expr,
        source: &str,
        code: &str,
    ) -> Option<&'a nodex_ir::Expr> {
        if &source[expr.span.start_byte as usize..expr.span.end_byte as usize] == code {
            Some(expr)
        } else {
            expr.children
                .iter()
                .find_map(|child| expression(child, source, code))
        }
    }
    for (code, expected) in [
        ("identity(1_i32)", scalar("Int32")),
        ("identity::<u64>(2)", scalar("UInt64")),
        ("first((3_i32, true))", scalar("Int32")),
        ("identity(4)", TypeShape::Unknown),
        ("value.get()", scalar("UInt64")),
        (r#"identity(c"example")"#, c_string_shape()),
    ] {
        let call = generic
            .nodes
            .iter()
            .filter_map(|node| node.expr.as_ref())
            .find_map(|expr| expression(expr, source, code))
            .unwrap_or_else(|| panic!("missing expression {code}"));
        assert_eq!(call.type_info.as_ref().unwrap().shape, expected, "{code}");
    }
}
