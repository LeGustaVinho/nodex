//! Framework knowledge: React, Vue, Angular and Node.js.
//!
//! Like Unity for C#, frameworks call user code at moments the code itself does not show. Nodex
//! makes those moments visible with the shared vocabulary:
//! - lifecycle methods and components become `EventEntry` Blueprints ("React renders <App>");
//! - callbacks handed to the framework (`useEffect`, `onMounted`, `app.get('/x', …)`) get their
//!   own Blueprint, linked from the statement that registers them (`Subscribe`);
//! - framework calls get labels that say what they mean (`useState`, `ref`, `inject`).

use tree_sitter::Node;

use crate::syntax::{Src, named_children, string_value};

/// The frameworks a file uses, from its imports (and its extension: `.vue`, JSX files).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Frameworks {
    pub react: bool,
    pub vue: bool,
    pub angular: bool,
    pub node: bool,
}

const NODE_MODULES: &[&str] = &[
    "fs", "path", "http", "https", "http2", "net", "os", "child_process", "worker_threads", "events", "stream", "crypto",
    "url", "util", "readline", "cluster", "zlib", "express", "fastify", "koa", "@koa/router", "@nestjs/core", "hono",
];

fn imports(text: &str, module: &str) -> bool {
    ['"', '\''].iter().any(|q| {
        text.contains(&format!("{q}{module}{q}")) || text.contains(&format!("{q}{module}/"))
    })
}

pub fn detect(rel: &str, text: &str) -> Frameworks {
    let jsx_file = rel.ends_with(".tsx") || rel.ends_with(".jsx");
    Frameworks {
        react: imports(text, "react") || imports(text, "react-dom") || imports(text, "preact") || imports(text, "next") || jsx_file,
        vue: rel.ends_with(".vue") || imports(text, "vue") || imports(text, "pinia") || imports(text, "vue-router"),
        angular: imports(text, "@angular/core") || text.contains("'@angular/") || text.contains("\"@angular/"),
        node: NODE_MODULES.iter().any(|m| imports(text, m) || imports(text, &format!("node:{m}")))
            || text.contains("require(\"node:")
            || text.contains("require('node:")
            || text.contains("process.argv")
            || text.contains("process.exit("),
    }
}

// -------------------------------------------------------------------------------------------------
// Lifecycles
// -------------------------------------------------------------------------------------------------

const REACT_CLASS: &[(&str, &str)] = &[
    ("render", "React calls render every time the component renders"),
    ("componentDidMount", "React calls componentDidMount after the component is first added to the page"),
    ("componentDidUpdate", "React calls componentDidUpdate after every re-render caused by new props or state"),
    ("componentWillUnmount", "React calls componentWillUnmount right before the component is removed"),
    ("shouldComponentUpdate", "React calls shouldComponentUpdate before re-rendering, to ask whether it is needed"),
    ("getDerivedStateFromProps", "React calls getDerivedStateFromProps before every render, to derive state from props"),
    ("getSnapshotBeforeUpdate", "React calls getSnapshotBeforeUpdate right before it updates the page"),
    ("componentDidCatch", "React calls componentDidCatch when a child component throws an error"),
    ("getDerivedStateFromError", "React calls getDerivedStateFromError when a child component throws an error"),
];

const VUE_OPTIONS: &[(&str, &str)] = &[
    ("beforeCreate", "Vue calls beforeCreate before the component's data exists"),
    ("created", "Vue calls created once the component's data and methods exist"),
    ("beforeMount", "Vue calls beforeMount right before the component is added to the page"),
    ("mounted", "Vue calls mounted after the component is added to the page"),
    ("beforeUpdate", "Vue calls beforeUpdate before the page is updated after a data change"),
    ("updated", "Vue calls updated after the page is updated after a data change"),
    ("beforeUnmount", "Vue calls beforeUnmount right before the component is removed"),
    ("unmounted", "Vue calls unmounted after the component is removed"),
    ("beforeDestroy", "Vue calls beforeDestroy right before the component is removed (Vue 2)"),
    ("destroyed", "Vue calls destroyed after the component is removed (Vue 2)"),
    ("activated", "Vue calls activated when a kept-alive component is shown again"),
    ("deactivated", "Vue calls deactivated when a kept-alive component is hidden"),
    ("errorCaptured", "Vue calls errorCaptured when a child component throws an error"),
    ("data", "Vue calls data to create the component's reactive data"),
    ("setup", "Vue calls setup once, when it creates the component"),
];

const VUE_HOOKS: &[(&str, &str)] = &[
    ("onBeforeMount", "right before the component is added to the page"),
    ("onMounted", "after the component is added to the page"),
    ("onBeforeUpdate", "before the page is updated after a data change"),
    ("onUpdated", "after the page is updated after a data change"),
    ("onBeforeUnmount", "right before the component is removed"),
    ("onUnmounted", "after the component is removed"),
    ("onActivated", "when a kept-alive component is shown again"),
    ("onDeactivated", "when a kept-alive component is hidden"),
    ("onErrorCaptured", "when a child component throws an error"),
];

const ANGULAR: &[(&str, &str)] = &[
    ("ngOnChanges", "Angular calls ngOnChanges when an input property changes"),
    ("ngOnInit", "Angular calls ngOnInit once, after the first input values are set"),
    ("ngDoCheck", "Angular calls ngDoCheck during every change detection"),
    ("ngAfterContentInit", "Angular calls ngAfterContentInit once, after the projected content is ready"),
    ("ngAfterContentChecked", "Angular calls ngAfterContentChecked after every check of the projected content"),
    ("ngAfterViewInit", "Angular calls ngAfterViewInit once, after the component's view is ready"),
    ("ngAfterViewChecked", "Angular calls ngAfterViewChecked after every check of the component's view"),
    ("ngOnDestroy", "Angular calls ngOnDestroy right before it destroys the component"),
];

/// Angular class decorators → flavor.
const ANGULAR_CLASSES: &[(&str, &str)] = &[
    ("Component", "angular:component"),
    ("Directive", "angular:directive"),
    ("Pipe", "angular:pipe"),
    ("Injectable", "angular:service"),
    ("NgModule", "angular:module"),
];

/// Angular member decorators → flavor.
const ANGULAR_MEMBERS: &[(&str, &str)] = &[
    ("Input", "angular:input"),
    ("Output", "angular:output"),
    ("ViewChild", "angular:view-query"),
    ("ViewChildren", "angular:view-query"),
    ("ContentChild", "angular:content-query"),
    ("ContentChildren", "angular:content-query"),
    ("HostBinding", "angular:host-binding"),
    ("HostListener", "angular:host-listener"),
];

pub fn react_class_base(base: &str) -> bool {
    matches!(base.rsplit('.').next().unwrap_or(base), "Component" | "PureComponent")
}

pub fn react_lifecycle(name: &str) -> Option<&'static str> {
    REACT_CLASS.iter().find(|(n, _)| *n == name).map(|(_, d)| *d)
}

pub fn vue_option(name: &str) -> Option<&'static str> {
    VUE_OPTIONS.iter().find(|(n, _)| *n == name).map(|(_, d)| *d)
}

pub fn angular_lifecycle(name: &str) -> Option<&'static str> {
    ANGULAR.iter().find(|(n, _)| *n == name).map(|(_, d)| *d)
}

/// Framework flavors implied by decorator flavors (`attr:Component` → `angular:component`).
pub fn angular_flavors(flavors: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for f in flavors {
        let Some(name) = f.strip_prefix("attr:") else { continue };
        let short = name.rsplit('.').next().unwrap_or(name);
        for (d, flavor) in ANGULAR_CLASSES.iter().chain(ANGULAR_MEMBERS) {
            if *d == short && !out.iter().any(|o: &String| o == flavor) {
                out.push((*flavor).to_owned());
            }
        }
    }
    out
}

/// `@HostListener('window:resize', ['$event'])` → the event label of the method it decorates.
pub fn host_listener(src: Src, decorator: Node) -> Option<String> {
    let call = named_children(decorator).into_iter().next().filter(|c| c.kind() == "call_expression")?;
    let callee = call.child_by_field_name("function").map(|f| src.norm(f))?;
    if callee.rsplit('.').next() != Some("HostListener") {
        return None;
    }
    let event = call.child_by_field_name("arguments").and_then(|a| named_children(a).into_iter().next()).map(|e| string_value(src, e))?;
    Some(format!("Angular calls this when the host element receives '{event}'"))
}

/// A React function component: PascalCase name and JSX in its body.
pub fn is_component_name(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

/// A React hook: `useSomething`.
pub fn is_hook_name(name: &str) -> bool {
    name.strip_prefix("use").is_some_and(|rest| rest.chars().next().is_some_and(char::is_uppercase))
}

/// Wrappers whose function argument is the real function of a constant
/// (`const Row = memo(function Row() { … })`, `const onSave = useCallback(() => { … }, [])`).
pub fn function_wrapper(callee: &str) -> Option<&'static str> {
    match callee.rsplit('.').next().unwrap_or(callee) {
        "memo" => Some("react:memo"),
        "forwardRef" => Some("react:forward-ref"),
        "useCallback" => Some("react:callback"),
        _ => None,
    }
}

/// Functions a component template calls on events: Vue `@click="add"` / `v-on:click="add(x)"`,
/// Angular `(click)="choose(hero)"` (in `.vue` files and inline Angular templates).
pub fn template_handlers(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let ident = |s: &str| -> String { s.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$').collect() };
    let mut push = |event: &str, handler: &str| {
        let name = ident(handler.trim());
        if !name.is_empty() && !event.is_empty() && !out.iter().any(|(n, e): &(String, String)| *n == name && e == event) {
            out.push((name, event.to_owned()));
        }
    };
    for (i, _) in text.match_indices("=\"") {
        // Attribute name before `="`.
        let start = text[..i].rfind(|c: char| c.is_whitespace() || c == '<').map(|p| p + 1).unwrap_or(0);
        let attr = &text[start..i];
        let value_end = text[i + 2..].find('"').map(|e| i + 2 + e).unwrap_or(i + 2);
        let value = &text[i + 2..value_end];
        let event = if let Some(e) = attr.strip_prefix('@') {
            Some(e)
        } else if let Some(e) = attr.strip_prefix("v-on:") {
            Some(e)
        } else if attr.starts_with('(') && attr.ends_with(')') && !attr.starts_with("([") {
            Some(&attr[1..attr.len() - 1])
        } else {
            None
        };
        if let Some(event) = event {
            let event = event.split('.').next().unwrap_or(event);
            push(event, value);
        }
    }
    out
}

// -------------------------------------------------------------------------------------------------
// Callbacks handed to a framework
// -------------------------------------------------------------------------------------------------

/// A callback registered with a framework or an event source.
pub struct Registration<'t> {
    /// The function value that the framework calls.
    pub callback: Node<'t>,
    /// Name of the callback's Blueprint (`effect`, `GET /users`, `on 'data'`).
    pub name: String,
    /// Label of the callback's `EventEntry` (`when a GET /users request arrives`).
    pub event: String,
    /// Label of the registering statement (`on GET /users requests, run …`).
    pub statement: String,
    pub flavor: &'static str,
}

const HTTP_VERBS: &[&str] = &["get", "post", "put", "patch", "delete", "del", "all", "options", "head"];

fn is_function_value(node: Node) -> bool {
    matches!(node.kind(), "arrow_function" | "function_expression" | "function" | "generator_function")
}

/// Recognizes a call that registers a callback: React effects, Vue lifecycle hooks and watchers,
/// Angular effects, RxJS subscriptions, HTTP routes and servers, and event listeners.
pub fn registration<'t>(src: Src, fw: Frameworks, call: Node<'t>) -> Option<Registration<'t>> {
    let func = call.child_by_field_name("function")?;
    let args: Vec<Node<'t>> = call.child_by_field_name("arguments").map(named_children).unwrap_or_default();
    let (name, target) = match func.kind() {
        "member_expression" => (
            func.child_by_field_name("property").map(|p| src.norm(p)).unwrap_or_default(),
            func.child_by_field_name("object").map(|o| src.norm(o)),
        ),
        "identifier" => (src.norm(func), None),
        _ => return None,
    };
    let unparen = |n: Node<'t>| crate::expr::unparen(n);
    let function_arg = |i: usize| args.get(i).copied().map(unparen).filter(|a| is_function_value(*a));
    let last_function = || args.last().copied().map(unparen).filter(|a| is_function_value(*a));
    let text = |n: Node| nodex_lang_api::kit::truncate(&src.norm(n), 40);
    let quoted = |n: Node| if n.kind() == "string" { format!("'{}'", string_value(src, n)) } else { text(n) };

    // React effects.
    if fw.react && matches!(name.as_str(), "useEffect" | "useLayoutEffect" | "useInsertionEffect") {
        let callback = function_arg(0)?;
        let when = match args.get(1) {
            None => "after every render".to_owned(),
            Some(deps) if deps.kind() == "array" && named_children(*deps).is_empty() => "once, after the first render".to_owned(),
            Some(deps) => {
                let list = src.norm(*deps).trim_matches(['[', ']']).trim().to_owned();
                format!("after rendering, when {list} {}", if list.contains(',') { "change" } else { "changes" })
            }
        };
        return Some(Registration {
            callback,
            name: if name == "useEffect" { "effect".into() } else { name.trim_start_matches("use").to_lowercase() },
            event: format!("React runs this {when}"),
            statement: format!("{when}, run the effect"),
            flavor: "react:effect",
        });
    }
    // Vue Composition API.
    if fw.vue && target.is_none() {
        if let Some((_, when)) = VUE_HOOKS.iter().find(|(n, _)| *n == name) {
            return Some(Registration {
                callback: function_arg(0)?,
                name: name.clone(),
                event: format!("Vue calls this {when}"),
                statement: format!("{when}, run …"),
                flavor: "vue:lifecycle",
            });
        }
        if name == "watch" {
            let source = args.first().map(|s| text(*s)).unwrap_or_default();
            return Some(Registration {
                callback: function_arg(1)?,
                name: format!("watch {source}"),
                event: format!("Vue runs this when {source} changes"),
                statement: format!("when {source} changes, run …"),
                flavor: "vue:watch",
            });
        }
    }
    if (fw.vue && name == "watchEffect" || fw.angular && name == "effect") && target.is_none() {
        return Some(Registration {
            callback: function_arg(0)?,
            name: name.clone(),
            event: "runs right away, then again whenever a value it reads changes".into(),
            statement: "run now and again whenever a value it reads changes".into(),
            flavor: if fw.vue { "vue:watch" } else { "angular:effect" },
        });
    }
    let target = target?;
    // RxJS (Angular services, any observable).
    if name == "subscribe" {
        return Some(Registration {
            callback: function_arg(0)?,
            name: format!("on {target} value"),
            event: format!("runs for every value {target} emits"),
            statement: format!("on every value of {target}, run …"),
            flavor: "rxjs:subscribe",
        });
    }
    // HTTP routes: `app.get('/users', handler)`, `router.post('/x', auth, handler)`.
    let path = args.first().filter(|a| a.kind() == "string").map(|a| string_value(src, *a));
    if HTTP_VERBS.contains(&name.as_str())
        && let Some(path) = path.as_ref().filter(|p| p.starts_with('/') || p.as_str() == "*")
    {
        let verb = if name == "del" { "DELETE".to_owned() } else { name.to_uppercase() };
        let verb = if verb == "ALL" { "any".to_owned() } else { verb };
        return Some(Registration {
            callback: last_function()?,
            name: format!("{verb} {path}"),
            event: format!("when a {verb} {path} request arrives"),
            statement: format!("on {verb} {path} requests, run …"),
            flavor: "node:route",
        });
    }
    if name == "use" && (fw.node || path.is_some()) {
        let scope = path.map(|p| format!(" under {p}")).unwrap_or_default();
        return Some(Registration {
            callback: last_function()?,
            name: format!("middleware{scope}"),
            event: format!("for every request{scope}, before its route"),
            statement: format!("for every request{scope}, first run …"),
            flavor: "node:middleware",
        });
    }
    if name == "listen" && fw.node {
        let port = args.first().filter(|a| !is_function_value(**a)).map(|a| text(*a));
        let on = port.map(|p| format!(" on port {p}")).unwrap_or_default();
        return Some(Registration {
            callback: last_function()?,
            name: "listening".into(),
            event: format!("when the server starts listening{on}"),
            statement: format!("start listening{on}, then run …"),
            flavor: "node:listen",
        });
    }
    if name == "createServer" {
        return Some(Registration {
            callback: last_function()?,
            name: "request handler".into(),
            event: "for every request the server receives".into(),
            statement: "create a server that runs … for every request".into(),
            flavor: "node:server",
        });
    }
    // Event listeners: `emitter.on('data', …)`, `process.on('SIGINT', …)`, `el.addEventListener('click', …)`.
    if matches!(name.as_str(), "on" | "once" | "addListener" | "addEventListener" | "prependListener") && args.len() >= 2 {
        let event = quoted(args[0]);
        return Some(Registration {
            callback: function_arg(1)?,
            name: format!("on {event}"),
            event: format!("when {target} raises {event}"),
            statement: format!("on {target} {event}, run …"),
            flavor: "event-handler",
        });
    }
    None
}

// -------------------------------------------------------------------------------------------------
// Statements that mean something specific in a framework
// -------------------------------------------------------------------------------------------------

/// What `const <name> = <call>(…)` means: label and flavor.
pub fn declaration(src: Src, fw: Frameworks, names: &[String], value: Node) -> Option<(String, &'static str)> {
    let call = crate::expr::unparen(value);
    if call.kind() != "call_expression" {
        return None;
    }
    let callee = call.child_by_field_name("function").map(|f| src.norm(f))?;
    let short = callee.rsplit('.').next().unwrap_or(&callee).split('<').next().unwrap_or("").to_owned();
    let args: Vec<Node> = call.child_by_field_name("arguments").map(named_children).unwrap_or_default();
    let arg = |i: usize| args.get(i).map(|a| nodex_lang_api::kit::truncate(&src.norm(*a), 40)).unwrap_or_default();
    let first = names.first().cloned().unwrap_or_default();
    let second = names.get(1).cloned();
    let deps = |i: usize| args.get(i).map(|d| src.norm(*d).trim_matches(['[', ']']).trim().to_owned()).unwrap_or_default();
    if fw.react {
        let meaning = match short.as_str() {
            "useState" => Some((
                match second {
                    Some(set) => format!("state {first}, changed with {set}; starts as {}", arg(0)),
                    None => format!("state {first}; starts as {}", arg(0)),
                },
                "react:state",
            )),
            "useReducer" => Some((
                match second {
                    Some(dispatch) => format!("state {first}, changed by sending actions to {dispatch} (reducer {})", arg(0)),
                    None => format!("state {first}, managed by reducer {}", arg(0)),
                },
                "react:state",
            )),
            "useRef" => Some((format!("{first} = a box kept between renders (.current starts as {})", arg(0)), "react:ref")),
            "useMemo" => Some((format!("{first} = remembered result, recomputed when {} {}", deps(1), plural(&deps(1))), "react:memo")),
            "useCallback" => Some((format!("{first} = remembered function, recreated when {} {}", deps(1), plural(&deps(1))), "react:memo")),
            "useContext" => Some((format!("{first} = the {} value from the nearest provider", arg(0)), "react:context")),
            "useId" => Some((format!("{first} = an id unique to this component"), "react:hook")),
            "useTransition" => Some((format!("{first} = whether a background update is running"), "react:hook")),
            s if is_hook_name(s) => Some((format!("{} = result of hook {s}({})", names.join(", "), arg(0)), "react:hook")),
            _ => None,
        };
        if meaning.is_some() {
            return meaning;
        }
    }
    if fw.vue {
        let meaning = match short.as_str() {
            "ref" | "shallowRef" => Some((format!("{first} = reactive value, starts as {} (read and write {first}.value)", arg(0)), "vue:ref")),
            "reactive" | "shallowReactive" => Some((format!("{first} = reactive object {}", arg(0)), "vue:reactive")),
            "computed" => Some((format!("{first} = computed value, recalculated when what it reads changes"), "vue:computed")),
            "defineProps" | "withDefaults" => Some((format!("{first} = the props the parent component passes"), "vue:props")),
            "defineEmits" => Some((format!("{first} = the events this component can raise to its parent"), "vue:emits")),
            "defineModel" => Some((format!("{first} = a value shared two-way with the parent (v-model)"), "vue:model")),
            "useRoute" | "useRouter" | "useStore" => Some((format!("{first} = {short}() from the app"), "vue:composable")),
            _ => None,
        };
        if meaning.is_some() {
            return meaning;
        }
    }
    if fw.node {
        let meaning = match short.as_str() {
            "spawn" | "exec" | "execFile" | "fork" => Some((format!("{first} = program {} running in another process", arg(0)), "node:process")),
            "express" | "fastify" | "Koa" | "Hono" => Some((format!("{first} = a new {short} web application"), "node:app")),
            "Router" => Some((format!("{first} = a group of routes"), "node:app")),
            s if s.ends_with("Sync") && s.len() > 4 => Some((format!("{first} = {} (blocks the program until done)", nodex_lang_api::kit::truncate(&src.norm(call), 60)), "node:blocking")),
            _ => None,
        };
        if meaning.is_some() {
            return meaning;
        }
    }
    if fw.angular {
        let meaning = match short.as_str() {
            "inject" => Some((format!("{first} = the {} that Angular provides (dependency injection)", arg(0)), "angular:inject")),
            "signal" => Some((format!("{first} = signal, starts as {} (read {first}(), change with .set)", arg(0)), "angular:signal")),
            "computed" => Some((format!("{first} = computed signal, recalculated when the signals it reads change"), "angular:signal")),
            "input" | "model" => Some((format!("{first} = input from the parent template"), "angular:input")),
            "output" => Some((format!("{first} = event the parent template can listen to"), "angular:output")),
            _ => None,
        };
        if meaning.is_some() {
            return meaning;
        }
    }
    None
}

fn plural(list: &str) -> &'static str {
    if list.contains(',') { "change" } else { "changes" }
}

/// A React state setter by convention: `setCount`, `setUser`.
pub fn is_setter_name(name: &str) -> bool {
    name.strip_prefix("set").is_some_and(|rest| rest.chars().next().is_some_and(char::is_uppercase))
}

/// Node.js calls with a specific meaning: `(concept, label, flavor)`.
pub fn node_call(src: Src, fw: Frameworks, call: Node) -> Option<(nodex_ir::Concept, String, &'static str)> {
    use nodex_ir::Concept;
    let callee = call.child_by_field_name("function").map(|f| src.norm(f))?;
    let short = callee.rsplit('.').next().unwrap_or(&callee).to_owned();
    let args: Vec<Node> = call.child_by_field_name("arguments").map(named_children).unwrap_or_default();
    let arg = |i: usize| args.get(i).map(|a| nodex_lang_api::kit::truncate(&src.norm(*a), 48)).unwrap_or_default();
    if callee == "process.exit" {
        let code = if args.is_empty() { "0".to_owned() } else { arg(0) };
        return Some((Concept::Return, format!("end the program with exit code {code}"), "node:exit"));
    }
    if !fw.node {
        return None;
    }
    if matches!(short.as_str(), "spawn" | "exec" | "execFile" | "fork" | "spawnSync" | "execSync" | "execFileSync") {
        let what = arg(0);
        let blocking = short.ends_with("Sync");
        return Some((
            Concept::Spawn,
            format!("run program {what}{}", if blocking { " and wait for it" } else { " in another process" }),
            if blocking { "node:blocking" } else { "node:process" },
        ));
    }
    if short.ends_with("Sync") && short.len() > 4 {
        return Some((Concept::Call, src.norm(call), "node:blocking"));
    }
    None
}
