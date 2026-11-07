//! Small helpers over the TypeScript/TSX tree-sitter trees.

use tree_sitter::Node;

pub use nodex_lang_api::syntax::{Src, children, children_of_kind, first_child_of_kind, first_error, named_children};

/// Nodes that start a new function scope (their bodies are not part of the enclosing body).
pub fn is_function(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration"
            | "generator_function_declaration"
            | "function_expression"
            | "function"
            | "generator_function"
            | "arrow_function"
            | "method_definition"
            | "class_declaration"
            | "abstract_class_declaration"
            | "class"
    )
}

/// Function values: what `const f = …` must hold to be a function.
pub fn is_function_value(kind: &str) -> bool {
    matches!(kind, "arrow_function" | "function_expression" | "function" | "generator_function")
}

/// True if `node` uses `this`, looking through arrow functions (which share `this`) but not
/// through `function`s, methods or classes (which have their own).
pub fn uses_this(node: Node) -> bool {
    named_children(node).into_iter().any(|c| {
        c.kind() == "this" || ((c.kind() == "arrow_function" || !is_function(c.kind())) && uses_this(c))
    })
}

/// Keyword tokens (`static`, `async`, `readonly`, `get`, `*`, ...) of a declaration, as flavors.
pub fn modifiers(src: Src, decl: Node) -> Vec<String> {
    let mut out = Vec::new();
    for c in children(decl) {
        let flavor = match c.kind() {
            "accessibility_modifier" | "override_modifier" => src.norm(c),
            _ if c.is_named() => continue,
            _ => match src.text(c) {
                "static" | "readonly" | "async" | "abstract" | "override" | "declare" | "accessor" | "const" => src.text(c).to_owned(),
                "*" => "generator".to_owned(),
                "?" => "optional".to_owned(),
                "get" => "getter".to_owned(),
                "set" => "setter".to_owned(),
                _ => continue,
            },
        };
        if !out.contains(&flavor) {
            out.push(flavor);
        }
    }
    // Modifiers the parser did not see (blanked by `prepare`, such as `abstract override`).
    if let Some(name) = decl.child_by_field_name("name").or_else(|| decl.child_by_field_name("pattern")) {
        let head = &src.text[decl.start_byte()..name.start_byte()];
        if has_word(head, "override") && !out.iter().any(|f| f == "override") {
            out.push("override".into());
        }
        for d in decorators_in_text(head) {
            if !out.contains(&d) {
                out.push(d);
            }
        }
    }
    out
}

/// True if `text` contains `word` as a whole word.
pub fn has_word(text: &str, word: &str) -> bool {
    let bytes = text.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
    text.match_indices(word).any(|(i, _)| {
        (i == 0 || !ident(bytes[i - 1])) && bytes.get(i + word.len()).is_none_or(|&b| !ident(b))
    })
}

/// `@Inject(TOKEN) private dep` → `attr:Inject`: decorators read from the source text (the
/// grammar does not know parameter decorators; `prepare` hides them from it).
pub fn decorators_in_text(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in text.match_indices('@') {
        let name: String = text[i + 1..].chars().take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | '.')).collect();
        if !name.is_empty() {
            out.push(format!("attr:{name}"));
        }
    }
    out
}

/// `@Component({...})` → `attr:Component`.
pub fn decorator_flavor(src: Src, decorator: Node) -> String {
    let expr = named_children(decorator).into_iter().next();
    let name = match expr {
        Some(e) if e.kind() == "call_expression" => e.child_by_field_name("function").map(|f| src.norm(f)).unwrap_or_default(),
        Some(e) => src.norm(e),
        None => src.norm(decorator).trim_start_matches('@').to_owned(),
    };
    format!("attr:{name}")
}

/// Decorators written directly on `node` (classes; class members carry theirs as preceding siblings).
pub fn decorators(src: Src, node: Node) -> Vec<String> {
    children_of_kind(node, "decorator").into_iter().map(|d| decorator_flavor(src, d)).collect()
}

/// The JSDoc block (`/** … */`) right before a declaration (or before its `export`), as plain
/// text without tags.
pub fn jsdoc(src: Src, node: Node) -> Option<String> {
    let anchor = match node.parent() {
        Some(p) if p.kind() == "export_statement" => p,
        _ => node,
    };
    let prev = anchor.prev_sibling()?;
    if prev.kind() != "comment" {
        return None;
    }
    let text = src.text(prev);
    let body = text.strip_prefix("/**")?.strip_suffix("*/")?;
    // The comment must end on the line before the declaration.
    let between = &src.text[prev.end_byte()..anchor.start_byte()];
    if between.matches('\n').count() > 1 {
        return None;
    }
    let mut lines = Vec::new();
    for line in body.lines() {
        let line = line.trim().trim_start_matches('*').trim();
        if line.starts_with('@') {
            break;
        }
        if !line.is_empty() {
            lines.push(line.to_owned());
        }
    }
    let doc = lines.join(" ");
    let doc = strip_inline_tags(&doc);
    (!doc.is_empty()).then_some(doc)
}

/// `{@link Foo}` → `Foo`.
fn strip_inline_tags(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("{@") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let inner = &after[..end];
        let target = inner.split_once(char::is_whitespace).map(|(_, t)| t).unwrap_or("");
        out.push_str(target.split('|').next().unwrap_or(target).trim());
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Identifiers bound by a pattern (`x`, `{ a, b: [c] }`, `...rest`, `x = 1`).
pub fn pattern_names(src: Src, pattern: Node, out: &mut Vec<String>) {
    match pattern.kind() {
        "identifier" | "shorthand_property_identifier_pattern" => out.push(src.norm(pattern)),
        "pair_pattern" => {
            if let Some(v) = pattern.child_by_field_name("value") {
                pattern_names(src, v, out);
            }
        }
        "assignment_pattern" | "object_assignment_pattern" => {
            if let Some(l) = pattern.child_by_field_name("left").or_else(|| named_children(pattern).into_iter().next()) {
                pattern_names(src, l, out);
            }
        }
        "object_pattern" | "array_pattern" | "rest_pattern" => {
            for c in named_children(pattern) {
                pattern_names(src, c, out);
            }
        }
        _ => {}
    }
}

/// `(name, type)` of each parameter of a function-like node. Destructured parameters bind each
/// name without a type; `this` parameters are skipped.
pub fn parameters(src: Src, func: Node) -> Vec<(String, Option<String>)> {
    if let Some(single) = func.child_by_field_name("parameter") {
        return vec![(src.norm(single), None)];
    }
    let Some(list) = func.child_by_field_name("parameters") else { return Vec::new() };
    let mut out = Vec::new();
    for p in named_children(list) {
        let (pattern, ty) = match p.kind() {
            "required_parameter" | "optional_parameter" => (
                p.child_by_field_name("pattern"),
                p.child_by_field_name("type").map(|t| type_text(src, t)),
            ),
            _ => (Some(p), None),
        };
        let Some(pattern) = pattern else { continue };
        if pattern.kind() == "this" {
            continue;
        }
        let mut names = Vec::new();
        pattern_names(src, pattern, &mut names);
        let simple = matches!(pattern.kind(), "identifier");
        let rest = pattern.kind() == "rest_pattern";
        for n in names {
            out.push((n, if simple || rest { ty.clone() } else { None }));
        }
    }
    out
}

/// `: number` → `number`.
pub fn type_text(src: Src, annotation: Node) -> String {
    src.norm(annotation).trim_start_matches(':').trim().to_owned()
}

/// Type names referenced inside a type (`Promise<Order[]>` → `Promise`, `Order`), skipping
/// built-in types.
pub fn type_names(src: Src, ty: Node, out: &mut Vec<String>) {
    match ty.kind() {
        "type_identifier" => {
            let name = src.norm(ty);
            if !out.contains(&name) {
                out.push(name);
            }
        }
        "nested_type_identifier" => {
            let name = src.norm(ty);
            if !out.contains(&name) {
                out.push(name);
            }
        }
        "object_type" | "literal_type" | "predefined_type" => {}
        _ => {
            for c in named_children(ty) {
                type_names(src, c, out);
            }
        }
    }
}

/// `Base<T>`, `mixin(Base)`, `ns.Base` → the name of the base type as written, without
/// generic arguments.
pub fn bare_type_name(text: &str) -> String {
    let text = text.trim();
    let end = text.find(['<', '(']).unwrap_or(text.len());
    text[..end].trim().to_owned()
}

/// The string value of a `string` node (`"./x"` → `./x`).
pub fn string_value(src: Src, node: Node) -> String {
    let t = src.text(node);
    t.trim_matches(['"', '\'', '`']).to_owned()
}

/// Statement node kinds lowered as statements inside a body.
pub fn is_statement(node: Node) -> bool {
    let k = node.kind();
    k != "comment" && (k.ends_with("_statement") || k.ends_with("_declaration") || matches!(k, "statement_block" | "ERROR" | "ambient_declaration" | "function_signature"))
}
