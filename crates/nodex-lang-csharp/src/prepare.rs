//! Prepares C# text for the tree-sitter grammar without moving any byte.
//!
//! The grammar predates some syntax (C# 13/14) and cannot parse conditional compilation that
//! splits a construct (`#if X public #endif static class`). Nodex parses a *prepared* copy of the
//! text in which such spots are rewritten to something the grammar accepts **with the same byte
//! length**, while labels, code and spans are always read from the original text. Every rule is
//! kept only if it reduces the number of parse errors.

use std::collections::HashMap;

use tree_sitter::{InputEdit, Node, Parser, Point, Tree};

pub struct Prepared {
    /// Text to parse (same length as the original). `None` when no rewrite was needed.
    pub text: Option<String>,
    /// The tree of the original text, when no rewrite was needed (a fresh parse, so the caller
    /// need not parse the file a second time).
    pub tree: Option<Tree>,
    /// C# 14 `extension(T r)` blocks rewritten as classes: start byte → receiver text.
    pub extension_blocks: HashMap<usize, String>,
    /// `#if` groups that split a declaration or statement and were resolved for the parser.
    pub merged_directives: Vec<DirectiveFix>,
}

/// How a conditional-compilation group that the grammar could not parse was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveFix {
    /// 1-based, inclusive line range of the `#if … #endif` group.
    pub first: usize,
    pub last: usize,
    /// `merged` (all variants shown together), or the directive of the single variant kept
    /// (`#if ODIN_INSPECTOR`, `#else`).
    pub kept: String,
}

/// Directive chains tried one by one, in a row without success, before the all-at-once pass.
const MAX_FAILED_CHAINS: usize = 6;

pub fn prepare(parser: &mut Parser, original: &str) -> Prepared {
    let mut out = Prepared { text: None, tree: None, extension_blocks: HashMap::new(), merged_directives: Vec::new() };
    let mut text = original.as_bytes().to_vec();

    // A UTF-8 byte order mark is not C#: the grammar reports it as an error.
    if text.starts_with(&[0xEF, 0xBB, 0xBF]) {
        text[..3].copy_from_slice(b"   ");
    }

    let Some(tree) = parse(parser, &text) else { return finish(out, text, original, None) };
    let mut errors = error_count(&tree);
    // The tree of `text`: candidates differ from it in a few same-length ranges, so they are
    // parsed incrementally from it instead of from scratch.
    let mut base = tree;
    if errors == 0 {
        return finish(out, text, original, Some(base));
    }
    // Kept while no rewrite is accepted: later trees are incremental parses of rewritten text.
    let fresh = base.clone();

    // C# 14 extension blocks: the grammar reads them as a constructor named `extension`.
    let (candidate, blocks) = rewrite_extension_blocks(&text);
    if !blocks.is_empty()
        && let Some(t) = reparse(parser, &text, &base, &candidate) {
            let e = error_count(&t);
            if e < errors {
                text = candidate;
                errors = e;
                base = t;
                out.extension_blocks = blocks;
            }
        }

    // Line-level rules on the lines that still fail, then directive merging as a last resort.
    for _ in 0..12 {
        if errors == 0 {
            break;
        }
        let lines = error_lines(&base);
        let mut improved = false;
        for rule in [null_conditional_assignment as Rule, compound_operator, allows_ref_struct, unary_dereference] {
            let candidate = apply_on_lines(&text, &lines, rule);
            if candidate == text {
                continue;
            }
            if let Some(t) = reparse(parser, &text, &base, &candidate) {
                let e = error_count(&t);
                if e < errors {
                    text = candidate;
                    errors = e;
                    base = t;
                    improved = true;
                }
            }
        }
        if improved {
            continue;
        }
        // Conditional compilation that splits a construct: for the innermost groups around the
        // failing lines, try merging the variants, keeping only the first, or only the last.
        let groups = directive_groups(&text);
        let mut progressed = false;
        let mut done: Vec<usize> = Vec::new();
        // Chains already tried without success since the text last changed. An ERROR node can
        // span hundreds of lines inside the same groups; each chain is worth one try, not one per
        // line (a 39 KB DOTween file took 8 s re-trying the same groups for 536 lines).
        let mut tried: std::collections::HashSet<Vec<usize>> = std::collections::HashSet::new();
        let mut failed_chains = 0;
        for &line in &lines {
            // Groups around the failing line, innermost first. Nested groups (an `#if` inside an
            // `#if`) are resolved together, widening until the error goes away.
            let mut chain: Vec<&DirectiveGroup> = groups.iter().filter(|g| g.first <= line && line <= g.last && !done.contains(&g.first)).collect();
            chain.sort_by_key(|g| g.last - g.first);
            if chain.is_empty() || !tried.insert(chain.iter().map(|g| g.first).collect()) {
                continue;
            }
            // Many groups that each fail alone usually only parse together (DOTween's marker
            // blocks): stop trying them one by one and let the all-at-once pass below run.
            if failed_chains >= MAX_FAILED_CHAINS {
                break;
            }
            let mut best: Option<(usize, Vec<u8>, Vec<DirectiveFix>, Tree)> = None;
            'levels: for depth in 1..=chain.len() {
                for strategy in [Strategy::Merge, Strategy::KeepFirst, Strategy::KeepLast] {
                    let outer = chain[depth - 1];
                    let mut candidate = text.clone();
                    let mut fixes = Vec::new();
                    // Every group nested inside the outer one is merged; the outer one uses the strategy.
                    let nested = groups.iter().filter(|h| h.first > outer.first && h.last < outer.last && !done.contains(&h.first));
                    for g in nested {
                        candidate = resolve_group(&candidate, g, Strategy::Merge);
                        fixes.push(DirectiveFix { first: g.first, last: g.last, kept: "merged".into() });
                    }
                    candidate = resolve_group(&candidate, outer, strategy);
                    fixes.push(DirectiveFix { first: outer.first, last: outer.last, kept: strategy.describe(&text, outer) });
                    if let Some(t) = reparse(parser, &text, &base, &candidate) {
                        let e = error_count(&t);
                        if e < errors && best.as_ref().is_none_or(|(b, _, _, _)| e < *b) {
                            best = Some((e, candidate, fixes, t));
                        }
                    }
                }
                if best.is_some() {
                    break 'levels;
                }
            }
            if let Some((e, candidate, fixes, t)) = best {
                errors = e;
                text = candidate;
                base = t;
                progressed = true;
                tried.clear();
                failed_chains = 0;
                done.extend(fixes.iter().map(|f| f.first));
                out.merged_directives.extend(fixes);
            } else {
                failed_chains += 1;
            }
        }
        if !progressed {
            // Several broken groups may hide behind one error: resolve every group that contains
            // a failing line (with the groups nested in it) at once.
            let outers: Vec<&DirectiveGroup> = groups
                .iter()
                .filter(|g| !done.contains(&g.first) && lines.iter().any(|&l| g.first <= l && l <= g.last))
                .collect();
            for strategy in [Strategy::Merge, Strategy::KeepFirst, Strategy::KeepLast] {
                let mut candidate = text.clone();
                let mut fixes = Vec::new();
                for outer in &outers {
                    for g in groups.iter().filter(|h| h.first > outer.first && h.last < outer.last) {
                        candidate = resolve_group(&candidate, g, Strategy::Merge);
                    }
                    candidate = resolve_group(&candidate, outer, strategy);
                    fixes.push(DirectiveFix { first: outer.first, last: outer.last, kept: strategy.describe(&text, outer) });
                }
                if let Some(t) = reparse(parser, &text, &base, &candidate)
                    && error_count(&t) < errors
                {
                    errors = error_count(&t);
                    text = candidate;
                    base = t;
                    progressed = true;
                    done.extend(fixes.iter().map(|f| f.first));
                    out.merged_directives.extend(fixes);
                    break;
                }
            }
        }
        if !progressed {
            break;
        }
    }
    finish(out, text, original, Some(fresh))
}

/// `tree` is a fresh parse of `text` before any rewrite; it is handed back only when the text
/// stayed the original.
fn finish(mut out: Prepared, text: Vec<u8>, original: &str, tree: Option<Tree>) -> Prepared {
    if text != original.as_bytes() {
        out.text = String::from_utf8(text).ok();
    } else {
        out.tree = tree;
    }
    out.merged_directives.sort_by_key(|d| d.first);
    out
}

fn parse(parser: &mut Parser, text: &[u8]) -> Option<Tree> {
    parser.parse(text, None)
}

/// Parses `candidate`, which has the same length as `old` (the text `tree` was parsed from),
/// reusing `tree` outside the range where the two differ.
fn reparse(parser: &mut Parser, old: &[u8], tree: &Tree, candidate: &[u8]) -> Option<Tree> {
    debug_assert_eq!(old.len(), candidate.len());
    let Some(start) = old.iter().zip(candidate).position(|(a, b)| a != b) else { return Some(tree.clone()) };
    let end = old.len() - old.iter().rev().zip(candidate.iter().rev()).position(|(a, b)| a != b).unwrap_or(0);
    let mut edited = tree.clone();
    edited.edit(&InputEdit {
        start_byte: start,
        old_end_byte: end,
        new_end_byte: end,
        start_position: point(old, start),
        old_end_position: point(old, end),
        new_end_position: point(candidate, end),
    });
    parser.parse(candidate, Some(&edited))
}

/// Row and byte column of `offset` in `text`.
fn point(text: &[u8], offset: usize) -> Point {
    let before = &text[..offset];
    let row = before.iter().filter(|&&b| b == b'\n').count();
    let column = offset - before.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    Point { row, column }
}

fn error_count(tree: &Tree) -> usize {
    let mut count = 0;
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        if n.is_error() || n.is_missing() {
            count += 1;
            continue;
        }
        if n.has_error() {
            let mut c = n.walk();
            stack.extend(n.children(&mut c));
        }
    }
    count
}

/// 1-based lines where parsing fails. An ERROR node may wrap a whole namespace, so this looks
/// inside it: nested failures, and the tokens the grammar could not place.
fn error_lines(tree: &Tree) -> Vec<usize> {
    let mut lines = Vec::new();
    let mut stack: Vec<Node> = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        let mut c = n.walk();
        let failing: Vec<Node> = n.children(&mut c).filter(|k| k.is_error() || k.is_missing() || k.has_error()).collect();
        if n.is_error() || n.is_missing() {
            lines.push(n.start_position().row + 1);
            let mut c = n.walk();
            for k in n.children(&mut c).filter(|k| !k.is_named()) {
                lines.push(k.start_position().row + 1);
            }
            if failing.is_empty() {
                lines.push(n.end_position().row + 1);
            }
        }
        stack.extend(failing);
    }
    lines.sort_unstable();
    lines.dedup();
    lines
}

type Rule = fn(&str) -> Option<String>;

/// Applies a same-length rewrite to the given lines (and their neighbours).
fn apply_on_lines(text: &[u8], lines: &[usize], rule: Rule) -> Vec<u8> {
    let Ok(s) = std::str::from_utf8(text) else { return text.to_vec() };
    let mut out = Vec::with_capacity(text.len());
    let wanted: Vec<usize> = lines.iter().flat_map(|&l| [l.saturating_sub(1), l, l + 1]).collect();
    for (i, line) in s.split_inclusive('\n').enumerate() {
        match wanted.contains(&(i + 1)).then(|| rule(line)).flatten() {
            Some(new) if new.len() == line.len() => out.extend_from_slice(new.as_bytes()),
            _ => out.extend_from_slice(line.as_bytes()),
        }
    }
    out
}

/// C# 14 `a?.B = c;` → `a .B = c;` (the original text still shows `?.`).
fn null_conditional_assignment(line: &str) -> Option<String> {
    if !(line.contains("?.") || line.contains("?[")) {
        return None;
    }
    let code = line.split("//").next().unwrap_or(line);
    let bytes = code.as_bytes();
    let has_assignment = (1..bytes.len().saturating_sub(1)).any(|i| {
        bytes[i] == b'='
            && !matches!(bytes[i - 1], b'=' | b'!' | b'<' | b'>')
            && !matches!(bytes[i + 1], b'=' | b'>')
    });
    has_assignment.then(|| line.replace("?.", " .").replace("?[", " ["))
}

/// C# 14 `operator +=(…)` → `operator + (…)`.
fn compound_operator(line: &str) -> Option<String> {
    let at = line.find("operator")?;
    let rest = &line[at + "operator".len()..];
    let trimmed = rest.trim_start();
    for op in [">>>=", "<<=", ">>=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^="] {
        if trimmed.starts_with(op) {
            let eq = at + "operator".len() + (rest.len() - trimmed.len()) + op.len() - 1;
            let mut s = line.to_owned();
            s.replace_range(eq..=eq, " ");
            return Some(s);
        }
    }
    None
}

/// C# 13 `where T : allows ref struct` → `where T : notnull` (padded).
fn allows_ref_struct(line: &str) -> Option<String> {
    line.contains("allows ref struct").then(|| line.replace("allows ref struct", "notnull          "))
}

/// Unary `*` (pointer dereference) the grammar rejects in some positions: `*(uint*)p`,
/// `*dst++ = *src++` → a space.
fn unary_dereference(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut s = bytes.to_vec();
    let mut changed = false;
    for i in 0..bytes.len() {
        if bytes[i] != b'*' {
            continue;
        }
        let next = bytes.get(i + 1).copied().unwrap_or(b' ');
        if !(next == b'(' || next.is_ascii_alphabetic() || next == b'_') {
            continue;
        }
        let prev = bytes[..i].iter().rev().find(|c| !c.is_ascii_whitespace()).copied();
        let unary = match prev {
            None => true,
            Some(c) => matches!(c, b'=' | b'(' | b',' | b';' | b'{' | b'}' | b'?' | b':' | b'+' | b'-' | b'&' | b'|' | b'!')
                || line[..i].trim_end().ends_with("return"),
        };
        if unary {
            s[i] = b' ';
            changed = true;
        }
    }
    changed.then(|| String::from_utf8(s).ok()).flatten()
}

#[derive(Clone)]
struct DirectiveGroup {
    first: usize,
    last: usize,
    directive_lines: Vec<usize>,
}

/// `#if … #elif … #else … #endif` groups with their directive lines (1-based).
fn directive_groups(text: &[u8]) -> Vec<DirectiveGroup> {
    let s = String::from_utf8_lossy(text);
    let mut groups = Vec::new();
    let mut stack: Vec<DirectiveGroup> = Vec::new();
    for (i, line) in s.lines().enumerate() {
        let t = line.trim_start();
        let n = i + 1;
        if t.starts_with("#if") {
            stack.push(DirectiveGroup { first: n, last: n, directive_lines: vec![n] });
        } else if t.starts_with("#elif") || t.starts_with("#else") {
            if let Some(g) = stack.last_mut() {
                g.directive_lines.push(n);
            }
        } else if t.starts_with("#endif")
            && let Some(mut g) = stack.pop() {
                g.directive_lines.push(n);
                g.last = n;
                groups.push(g);
            }
    }
    groups
}

#[derive(Clone, Copy)]
enum Strategy {
    Merge,
    KeepFirst,
    KeepLast,
}

impl Strategy {
    fn describe(self, text: &[u8], g: &DirectiveGroup) -> String {
        let s = String::from_utf8_lossy(text);
        let directive = |line: usize| s.lines().nth(line - 1).unwrap_or("").trim().to_owned();
        match self {
            Strategy::Merge => "merged".to_owned(),
            Strategy::KeepFirst => directive(g.directive_lines[0]),
            Strategy::KeepLast => directive(g.directive_lines[g.directive_lines.len().saturating_sub(2)]),
        }
    }
}

/// Blanks a group's directives and, unless merging, the content of the variants not kept.
fn resolve_group(text: &[u8], g: &DirectiveGroup, strategy: Strategy) -> Vec<u8> {
    let mut out = text.to_vec();
    let d = &g.directive_lines;
    // Variant i spans the lines between directive i and directive i+1.
    let variants: Vec<(usize, usize)> = d.windows(2).map(|w| (w[0] + 1, w[1] - 1)).collect();
    let keep = match strategy {
        Strategy::Merge => None,
        Strategy::KeepFirst => Some(0),
        Strategy::KeepLast => Some(variants.len().saturating_sub(1)),
    };
    for &l in d {
        blank_line(&mut out, l);
    }
    if let Some(k) = keep {
        for (i, &(a, b)) in variants.iter().enumerate() {
            if i != k {
                for l in a..=b {
                    blank_line(&mut out, l);
                }
            }
        }
    }
    out
}

/// Replaces a 1-based line's content with spaces, keeping its length and newline.
fn blank_line(text: &mut [u8], line: usize) {
    let mut current = 1;
    let mut start = 0;
    for i in 0..=text.len() {
        if i == text.len() || text[i] == b'\n' {
            if current == line {
                for b in &mut text[start..i] {
                    if *b != b'\r' {
                        *b = b' ';
                    }
                }
                return;
            }
            current += 1;
            start = i + 1;
        }
    }
}

/// Finds `extension(…) {` at a member boundary and rewrites the header to `class ___…` of the
/// same byte length so the block parses as a nested type.
fn rewrite_extension_blocks(bytes: &[u8]) -> (Vec<u8>, HashMap<usize, String>) {
    let mut blocks = HashMap::new();
    let Ok(text) = std::str::from_utf8(bytes) else { return (bytes.to_vec(), blocks) };
    let mut out = bytes.to_vec();
    let mut search = 0;
    while let Some(pos) = text[search..].find("extension") {
        let at = search + pos;
        search = at + "extension".len();
        let prev = last_code_char(&text[..at]);
        let boundary = matches!(prev, None | Some('{') | Some('}') | Some(';') | Some(']'));
        let word_start = at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
        let rest = &text[search..];
        let gap = rest.len() - rest.trim_start().len();
        if !boundary || !word_start || !rest.trim_start().starts_with('(') {
            continue;
        }
        let mut depth = 0usize;
        let mut close = None;
        for (i, ch) in rest[gap..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(search + gap + i);
                        break;
                    }
                }
                '{' | ';' | '}' => break,
                _ => {}
            }
        }
        let Some(close) = close else { continue };
        if !text[close + 1..].trim_start().starts_with('{') || close + 1 - at < 9 {
            continue;
        }
        let receiver = text[search + gap + 1..close].trim().to_owned();
        let replacement = format!("class {}", "_".repeat(close + 1 - at - 6));
        out[at..=close].copy_from_slice(replacement.as_bytes());
        blocks.insert(at, receiver);
    }
    (out, blocks)
}

/// The last character of `text` that is not whitespace or inside a trailing comment.
fn last_code_char(mut text: &str) -> Option<char> {
    loop {
        text = text.trim_end();
        if let Some(stripped) = text.strip_suffix("*/") {
            text = &stripped[..stripped.rfind("/*").unwrap_or(0)];
            continue;
        }
        let line_start = text.rfind('\n').map_or(0, |i| i + 1);
        if text[line_start..].trim_start().starts_with("//") {
            text = &text[..line_start];
            continue;
        }
        return text.chars().last();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser() -> Parser {
        let mut p = Parser::new();
        p.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
        p
    }

    fn errors_after(src: &str) -> (usize, Prepared) {
        let mut p = parser();
        let prepared = prepare(&mut p, src);
        let text = prepared.text.clone().unwrap_or_else(|| src.to_owned());
        assert_eq!(text.len(), src.len(), "rewrites keep the length");
        (error_count(&p.parse(&text, None).unwrap()), prepared)
    }

    #[test]
    fn bom_is_neutralized() {
        assert_eq!(errors_after("\u{feff}namespace A { class B {} }").0, 0);
    }

    #[test]
    fn directives_splitting_a_declaration_are_merged() {
        let src = "namespace A {\n#if X\n    public\n#endif\n    static class B { }\n}\n";
        let (errors, prepared) = errors_after(src);
        assert_eq!(errors, 0);
        assert_eq!(prepared.merged_directives, vec![DirectiveFix { first: 2, last: 4, kept: "merged".into() }]);
    }

    #[test]
    fn directives_around_cases_and_else_if_are_merged() {
        let src = "class C { int M(int x) {\n switch (x) {\n#if X\n case 1:\n return 1;\n#endif\n default: return 0; }\n if (x > 0) { }\n#if Y\n else if (x < 0) { }\n#endif\n else { }\n return 2; } }";
        assert_eq!(errors_after(src).0, 0);
    }

    #[test]
    fn nested_directives_around_attributes_are_resolved_together() {
        let src = "class C<T> : I<T> {
#if A 
    [X]
    [Z]
#if B
    [Y(After = \"q\")]
#endif
#endif
    private readonly D<T, int> f;
}
";
        let (errors, prepared) = errors_after(src);
        assert_eq!(errors, 0);
        assert!(!prepared.merged_directives.is_empty());
    }

    #[test]
    fn alternative_headers_keep_one_variant() {
        let src = "namespace A {\n    public class B :\n#if X\n        C\n#else\n        D\n#endif\n    { }\n}\n";
        let (errors, prepared) = errors_after(src);
        assert_eq!(errors, 0);
        assert_eq!(prepared.merged_directives[0].kept, "#if X");
    }

    #[test]
    fn csharp_14_and_pointer_rules() {
        assert_eq!(errors_after("class C { void M(C? c) { c?.F = 5; } int F; }").0, 0);
        assert_eq!(errors_after("struct M { public void operator +=(M o) { } }").0, 0);
        assert_eq!(errors_after("class C { static void M<T>(T v) where T : allows ref struct { } }").0, 0);
        assert_eq!(errors_after("unsafe class C { uint M(byte* p) { return *(uint*) p; } }").0, 0);
    }
}
