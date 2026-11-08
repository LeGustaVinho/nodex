//! Helpers that keep generated Mermaid text valid: synthetic ids, escaped labels, styles.

use std::collections::HashMap;

/// Synthetic element ids (`n0`, `n1`, ...) in the order elements are first named. Real names
/// never become ids, so reserved words (`end`, `class`, `graph`) and punctuation cannot break
/// the text.
#[derive(Default)]
pub struct Ids {
    ids: HashMap<String, String>,
    next: usize,
    prefix: &'static str,
}

impl Ids {
    pub fn new(prefix: &'static str) -> Self {
        Self { ids: HashMap::new(), next: 0, prefix }
    }

    pub fn id(&mut self, key: &str) -> String {
        if let Some(id) = self.ids.get(key) {
            return id.clone();
        }
        let id = format!("{}{}", self.prefix, self.next);
        self.next += 1;
        self.ids.insert(key.to_owned(), id.clone());
        id
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.ids.get(key).map(String::as_str)
    }
}

/// A label for a quoted Mermaid string (`n0["..."]`, `-->|"..."|`): quotes, angle brackets and
/// `#` become entity codes; long text is shortened.
pub fn label(text: &str, max: usize) -> String {
    let short = shorten(text, max);
    let mut out = String::with_capacity(short.len());
    for ch in short.chars() {
        match ch {
            '#' => out.push_str("#35;"),
            '"' => out.push_str("#quot;"),
            '<' => out.push_str("#lt;"),
            '>' => out.push_str("#gt;"),
            '\n' | '\r' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Several lines of a node label (`<br/>` is kept as markup).
pub fn lines(parts: &[String], max: usize) -> String {
    parts.iter().filter(|p| !p.is_empty()).map(|p| label(p, max)).collect::<Vec<_>>().join("<br/>")
}

/// Text inside a class body or after `:` on a relation, where quotes are not allowed:
/// generics become `~T~`, characters Mermaid reads as syntax are dropped.
pub fn class_text(text: &str, max: usize) -> String {
    let short = shorten(text, max);
    let mut out = String::with_capacity(short.len());
    for ch in short.chars() {
        match ch {
            '<' | '>' => out.push('~'),
            '"' | '{' | '}' | ';' | '`' | '#' | '\n' | '\r' => {}
            '[' => out.push('['),
            c => out.push(c),
        }
    }
    out.trim().to_owned()
}

pub fn shorten(text: &str, max: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= max {
        return text;
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Shared styles: elements outside the scope, external ones, warnings, infrastructure.
pub const CLASS_DEFS: &[(&str, &str)] = &[
    ("ctx", "fill:#1b1f26,stroke:#4b5563,color:#9aa4b2,stroke-dasharray:4 3"),
    ("ext", "fill:#1b1f26,stroke:#6b7280,color:#9aa4b2"),
    ("warn", "stroke:#e5534b,stroke-width:2px"),
    ("infra", "fill:#13202b,stroke:#58a6ff,color:#cfe3f7"),
    ("iface", "fill:#10231f,stroke:#39c5bb,color:#c8efe9"),
    ("test", "fill:#1f1a2b,stroke:#a371f7,color:#d9c9fb"),
    ("comp", "fill:#1d232c,stroke:#f5b041,color:#eef1f4"),
    ("pkg", "fill:#1d232c,stroke:#8b98a8,color:#eef1f4"),
    ("start", "fill:#eef1f4,stroke:#eef1f4,color:#0d1117"),
    ("final", "fill:#0d1117,stroke:#eef1f4,stroke-width:3px,color:#eef1f4"),
    ("err", "fill:#2b1416,stroke:#e5534b,color:#f6c9c6"),
    ("bar", "fill:#eef1f4,stroke:#eef1f4,color:#eef1f4"),
    ("hl", "stroke:#f5b041,stroke-width:3px"),
    ("actor", "fill:#1d232c,stroke:#f5b041,color:#eef1f4"),
    ("incl", "fill:#13202b,stroke:#58a6ff,color:#cfe3f7,stroke-dasharray:4 3"),
    ("hot", "stroke:#2ecc71,stroke-width:2.5px"),
    ("cold", "fill:#15181d,stroke:#3a434f,color:#6b7684"),
];

pub fn class_defs(out: &mut String, used: &[&str]) {
    for (name, style) in CLASS_DEFS {
        if used.contains(name) {
            out.push_str(&format!("  classDef {name} {style}\n"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_escape_what_mermaid_reads_as_syntax() {
        assert_eq!(label("a \"b\" <T> #1", 80), "a #quot;b#quot; #lt;T#gt; #35;1");
        assert_eq!(class_text("List<Order> \"x\" {y}", 80), "List~Order~ x y");
        assert_eq!(shorten("abcdef", 4), "abc…");
        let mut ids = Ids::new("n");
        assert_eq!(ids.id("end"), "n0");
        assert_eq!(ids.id("class"), "n1");
        assert_eq!(ids.id("end"), "n0");
    }
}
