//! A copy of the text that tree-sitter-typescript 0.23 can parse, with the same byte length
//! (see ADR-011). Labels, code and spans keep reading the original, so a rewrite only changes
//! what the parser sees.
//!
//! The byte order mark is always neutralized. The other rules bridge syntax the grammar
//! predates; each is applied only to a file that has parse errors, and kept only if it
//! reduces them.

use tree_sitter::{Parser, Tree};

/// A rewrite: `None` when it does not apply.
type Rule = fn(&str) -> Option<String>;

/// In application order.
const RULES: &[(&str, Rule)] = &[
    ("variance annotations", variance),
    ("export type *", export_type_star),
    ("abstract override", abstract_override),
    ("parameter decorators", parameter_decorators),
    ("using in for…of", for_using),
    ("using as a name", using_identifier),
    ("#private in", private_in),
    ("import types", import_types),
    ("anonymous default function signature", default_function_signature),
    ("modifier keywords as property names", keyword_property_names),
];

/// The parse of the best prepared copy, and the names of the rules it needed.
pub struct Prepared {
    pub tree: Tree,
    pub rules: Vec<&'static str>,
}

pub fn parse(parser: &mut Parser, text: &str) -> Option<Prepared> {
    let mut current = match text.strip_prefix('\u{feff}') {
        Some(rest) => format!("   {rest}"),
        None => text.to_owned(),
    };
    let mut tree = parser.parse(&current, None)?;
    let mut rules = Vec::new();
    if !tree.root_node().has_error() {
        return Some(Prepared { tree, rules });
    }
    let mut errors = error_count(&tree);
    for (name, rule) in RULES {
        let Some(candidate) = rule(&current) else { continue };
        debug_assert_eq!(candidate.len(), current.len());
        let Some(better) = parser.parse(&candidate, None) else { continue };
        let e = error_count(&better);
        if e < errors {
            current = candidate;
            tree = better;
            errors = e;
            rules.push(*name);
            if errors == 0 {
                break;
            }
        }
    }
    Some(Prepared { tree, rules })
}

pub fn error_count(tree: &Tree) -> usize {
    let mut count = 0;
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        if n.is_error() || n.is_missing() {
            count += 1;
        }
        if n.has_error() {
            let mut c = n.walk();
            stack.extend(n.children(&mut c));
        }
    }
    count
}

// ---------------------------------------------------------------------------------------------
// Scanning helpers
// ---------------------------------------------------------------------------------------------

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Byte offsets where the whole word `word` occurs.
fn words<'a>(text: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    let bytes = text.as_bytes();
    text.match_indices(word).map(|(i, _)| i).filter(move |&i| {
        (i == 0 || !is_ident(bytes[i - 1])) && bytes.get(i + word.len()).is_none_or(|&b| !is_ident(b))
    })
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// Skips whitespace and comments.
fn skip_trivia(bytes: &[u8], mut i: usize) -> usize {
    loop {
        i = skip_ws(bytes, i);
        if bytes.get(i) == Some(&b'/') && bytes.get(i + 1) == Some(&b'*') {
            match bytes[i + 2..].windows(2).position(|w| w == b"*/") {
                Some(p) => i += 2 + p + 2,
                None => return bytes.len(),
            }
        } else if bytes.get(i) == Some(&b'/') && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else {
            return i;
        }
    }
}

fn prev_non_ws(bytes: &[u8], i: usize) -> Option<(usize, u8)> {
    (0..i).rev().find(|&j| !bytes[j].is_ascii_whitespace()).map(|j| (j, bytes[j]))
}

fn ident_end(bytes: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < bytes.len() && is_ident(bytes[j]) {
        j += 1;
    }
    j
}

fn blank(out: &mut [u8], from: usize, to: usize) {
    for b in &mut out[from..to] {
        if !b.is_ascii_whitespace() {
            *b = b' ';
        }
    }
}

fn finish(out: Vec<u8>, changed: bool) -> Option<String> {
    changed.then(|| String::from_utf8(out).ok()).flatten()
}

// ---------------------------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------------------------

/// `interface Box<in out T>`, `class C<out O = unknown>` (TypeScript 4.7): the modifiers go.
fn variance(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut changed = false;
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'<' && b != b',' {
            continue;
        }
        let mut j = skip_trivia(bytes, i + 1);
        let mut modifiers = Vec::new();
        loop {
            let end = ident_end(bytes, j);
            let word = &text[j..end];
            if (word == "in" || word == "out") && end < bytes.len() && bytes[end].is_ascii_whitespace() {
                modifiers.push((j, end));
                j = skip_ws(bytes, end);
            } else {
                break;
            }
        }
        if modifiers.is_empty() {
            continue;
        }
        // A type parameter name follows, then `=`, `,`, `>` or `extends`.
        let name_end = ident_end(bytes, j);
        if name_end == j {
            continue;
        }
        let k = skip_ws(bytes, name_end);
        let follows = bytes.get(k).copied();
        if !(matches!(follows, Some(b'=' | b',' | b'>')) || text[k..].starts_with("extends")) {
            continue;
        }
        for (s, e) in modifiers {
            blank(&mut out, s, e);
        }
        changed = true;
    }
    finish(out, changed)
}

/// `export type * from "./types"` (TypeScript 5.0): `type` goes.
fn export_type_star(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut changed = false;
    for i in words(text, "export") {
        let t = skip_ws(bytes, i + 6);
        if !text[t..].starts_with("type") || bytes.get(t + 4).is_some_and(|&b| is_ident(b)) {
            continue;
        }
        if bytes.get(skip_ws(bytes, t + 4)) == Some(&b'*') {
            blank(&mut out, t, t + 4);
            changed = true;
        }
    }
    finish(out, changed)
}

/// `abstract override id: string;`: `override` goes.
fn abstract_override(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut changed = false;
    for i in words(text, "abstract") {
        let o = skip_ws(bytes, i + 8);
        if text[o..].starts_with("override") && bytes.get(o + 8).is_some_and(|b| b.is_ascii_whitespace()) {
            blank(&mut out, o, o + 8);
            changed = true;
        }
    }
    finish(out, changed)
}

/// `constructor(@Inject(TOKEN) private dep: Dep)`: decorators of parameters go.
fn parameter_decorators(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut changed = false;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'@' || !matches!(prev_non_ws(bytes, i), Some((_, b'(' | b','))) {
            i += 1;
            continue;
        }
        // `@name.path` then an optional balanced argument list.
        let mut j = i + 1;
        while j < bytes.len() && (is_ident(bytes[j]) || bytes[j] == b'.') {
            j += 1;
        }
        if j == i + 1 {
            i += 1;
            continue;
        }
        if bytes.get(j) == Some(&b'(') {
            let mut depth = 0;
            let mut quote = None;
            while j < bytes.len() {
                let c = bytes[j];
                match quote {
                    Some(q) if c == q && bytes[j - 1] != b'\\' => quote = None,
                    Some(_) => {}
                    None => match c {
                        b'"' | b'\'' | b'`' => quote = Some(c),
                        b'(' => depth += 1,
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                j += 1;
                                break;
                            }
                        }
                        _ => {}
                    },
                }
                j += 1;
            }
        }
        blank(&mut out, i, j);
        changed = true;
        i = j;
    }
    finish(out, changed)
}

/// `for (using r of rs)`, `for (await using r of rs)` (TypeScript 5.2): read as `const`.
fn for_using(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut changed = false;
    for i in words(text, "for") {
        let mut j = skip_ws(bytes, i + 3);
        if text[j..].starts_with("await") {
            j = skip_ws(bytes, j + 5);
        }
        if bytes.get(j) != Some(&b'(') {
            continue;
        }
        let mut k = skip_ws(bytes, j + 1);
        let await_at = (text[k..].starts_with("await") && bytes.get(k + 5).is_some_and(|b| b.is_ascii_whitespace())).then_some(k);
        if let Some(a) = await_at {
            k = skip_ws(bytes, a + 5);
        }
        if !text[k..].starts_with("using") || !bytes.get(k + 5).is_some_and(|b| b.is_ascii_whitespace()) {
            continue;
        }
        if let Some(a) = await_at {
            blank(&mut out, a, a + 5);
        }
        out[k..k + 5].copy_from_slice(b"const");
        changed = true;
    }
    finish(out, changed)
}

/// `using` as an ordinary name (`using.bind(…)`, `f(using, x)`, `(using: Node)`), which the
/// grammar reserves: read as `_sing`.
fn using_identifier(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut changed = false;
    for i in words(text, "using") {
        let next = bytes.get(skip_ws(bytes, i + 5)).copied();
        if matches!(next, Some(b'.' | b',' | b')' | b']' | b';' | b':' | b'=' | b'(' | b'[' | b'?' | b'}')) {
            out[i] = b'_';
            changed = true;
        }
    }
    finish(out, changed)
}

/// `#field in obj` (ergonomic brand checks, ES2022): read as `_field in obj`.
fn private_in(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut changed = false;
    for (i, _) in text.match_indices('#') {
        if i > 0 && (is_ident(bytes[i - 1]) || bytes[i - 1] == b'.') {
            continue;
        }
        let end = ident_end(bytes, i + 1);
        if end == i + 1 {
            continue;
        }
        let k = skip_ws(bytes, end);
        if k > end && text[k..].starts_with("in") && bytes.get(k + 2).is_some_and(|b| b.is_ascii_whitespace()) {
            out[i] = b'_';
            changed = true;
        }
    }
    finish(out, changed)
}

/// Import types (`x: import('./a').T`, `typeof import('node:fs')`, `() => import("x").T`):
/// the `import('…')` part becomes an identifier of the same length (`_____.T`).
fn import_types(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut changed = false;
    let mut from = 0;
    while let Some(pos) = text[from..].find("import(").map(|p| p + from) {
        from = pos + 7;
        let before = text[..pos].trim_end();
        let type_position = before.ends_with("typeof")
            || before.ends_with("readonly")
            || before.ends_with("keyof")
            || before.ends_with("=>")
            || before.ends_with([':', '<', '|', '&']);
        if !type_position || (pos > 0 && is_ident(bytes[pos - 1])) {
            continue;
        }
        let Some(close) = text[pos..].find(')').map(|c| c + pos) else { continue };
        let inner = text[pos + 7..close].trim();
        if !(inner.starts_with(['"', '\'']) && inner.ends_with(['"', '\''])) || text[pos..close].contains('\n') {
            continue;
        }
        for b in &mut out[pos..=close] {
            *b = b'_';
        }
        changed = true;
    }
    finish(out, changed)
}

/// `export default function (): T;` — the signature tsc writes into `.d.ts` files for an
/// anonymous default export — read as a named signature.
fn default_function_signature(text: &str) -> Option<String> {
    let mut out = text.as_bytes().to_vec();
    let mut changed = false;
    for (pattern, replacement) in [("default function (", "function default_("), ("default function(", "function defaul_(")] {
        for (i, _) in text.match_indices(pattern) {
            out[i..i + pattern.len()].copy_from_slice(replacement.as_bytes());
            changed = true;
        }
    }
    finish(out, changed)
}

/// `abstract?: boolean` or `{ declare: true }`: modifier keywords used as property names, which
/// the grammar rejects there: read as `_bstract`, `_eclare`.
fn keyword_property_names(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut changed = false;
    for word in ["abstract", "declare", "override", "accessor"] {
        for i in words(text, word) {
            let mut k = skip_ws(bytes, i + word.len());
            if bytes.get(k) == Some(&b'?') {
                k += 1;
            }
            let colon = bytes.get(k) == Some(&b':');
            let starts_member = matches!(prev_non_ws(bytes, i), Some((_, b'{' | b';' | b',')) | None);
            if colon && starts_member {
                out[i] = b'_';
                changed = true;
            }
        }
    }
    finish(out, changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same_len(rule: Rule, input: &str) -> String {
        let out = rule(input).unwrap_or_else(|| panic!("rule applies to {input}"));
        assert_eq!(out.len(), input.len());
        out
    }

    #[test]
    fn rules_keep_length_and_target_only_their_syntax() {
        assert_eq!(same_len(variance, "interface B<in out T> {}"), "interface B<       T> {}");
        assert_eq!(same_len(variance, "class C<out O = unknown, in I> {}"), "class C<    O = unknown,    I> {}");
        assert!(variance("for (const k in obj) {}").is_none());
        assert!(variance("interface O<\n  /** doc */\n  out S extends X = Y,\n> {}").is_some_and(|t| !t.contains("out S")));
        assert_eq!(same_len(keyword_property_names, "{ abstract?: boolean; declare: x }"), "{ _bstract?: boolean; _eclare: x }");
        assert!(keyword_property_names("abstract class X {}").is_none());
        assert_eq!(same_len(export_type_star, "export type * from './t'"), "export      * from './t'");
        assert_eq!(same_len(abstract_override, "abstract override id: T;"), "abstract          id: T;");
        assert_eq!(same_len(parameter_decorators, "constructor(@Inject(A) a, @B() b) {}"), format!("constructor({} a, {} b) {{}}", " ".repeat(10), " ".repeat(4)));
        assert!(parameter_decorators("f('@x', y)").is_none());
        assert_eq!(same_len(for_using, "for (await using r of rs) {}"), "for (      const r of rs) {}");
        assert_eq!(same_len(using_identifier, "using.bind(null); f(using, 1)"), "_sing.bind(null); f(_sing, 1)");
        assert!(using_identifier("using res = open();").is_none());
        assert_eq!(same_len(private_in, "if (#secret in o) {}"), "if (_secret in o) {}");
        assert!(private_in("this.#secret in o").is_none());
        assert_eq!(same_len(import_types, "f: () => import('x').T"), "f: () => ___________.T");
    }
}
