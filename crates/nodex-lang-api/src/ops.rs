//! Operators → shared operations of `catalog/operations.toml`.
//!
//! Every frontend lowers `a + b`, `!x`, `i++` and `total += x` to an [`Expr`], then calls
//! [`operator`] / [`compound_assign`] with the operator as written. The result names the catalog
//! key in `Expr::op`, so `a >= b` is the same *Greater or equal* node in every language.

use nodex_ir::{Concept, Expr};

/// How a language spells equality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Equality {
    /// `==` compares values (C#, Rust).
    Strict,
    /// `===` compares values; `==` converts types first (TypeScript, JavaScript).
    Loose,
}

/// Catalog key of a binary operator.
pub fn binary_key(op: &str, equality: Equality) -> Option<&'static str> {
    Some(match op {
        "+" => "add",
        "-" => "subtract",
        "*" => "multiply",
        "/" => "divide",
        "%" => "remainder",
        "**" => "power",
        "===" => "equal",
        "!==" => "not_equal",
        "==" if equality == Equality::Loose => "loose_equal",
        "!=" if equality == Equality::Loose => "loose_not_equal",
        "==" => "equal",
        "!=" => "not_equal",
        "<" => "less",
        "<=" => "less_or_equal",
        ">" => "greater",
        ">=" => "greater_or_equal",
        "&&" => "and",
        "||" => "or",
        "&" => "bit_and",
        "|" => "bit_or",
        "^" => "bit_xor",
        "<<" => "shift_left",
        ">>" => "shift_right",
        ">>>" => "shift_right_unsigned",
        _ => return None,
    })
}

/// Catalog key of a unary operator.
pub fn unary_key(op: &str) -> Option<&'static str> {
    Some(match op {
        "-" => "negate",
        "+" => "plus",
        "!" => "not",
        "~" => "bit_not",
        "++" => "increment",
        "--" => "decrement",
        _ => return None,
    })
}

/// Operators that change a variable; they stay on the execution chain.
fn is_impure(key: &str) -> bool {
    matches!(key, "increment" | "decrement")
}

/// Result type of an operation, when the operator alone tells it.
fn result_type(key: &str, children: &[Expr]) -> Option<&'static str> {
    match key {
        "equal" | "not_equal" | "loose_equal" | "loose_not_equal" | "less" | "less_or_equal" | "greater"
        | "greater_or_equal" | "is_missing" | "is_present" | "not" => Some("bool"),
        "and" | "or" if children.iter().all(|c| c.ty.as_deref() == Some("bool")) => Some("bool"),
        "and" | "or" => None,
        "text.concat" => Some("text"),
        "add" if children.iter().all(|c| c.ty.as_deref() == Some("number")) => Some("number"),
        "add" => None,
        _ => Some("number"),
    }
}

/// The type of a literal written `text` (`"a"`, `42`, `true`), when it is obvious.
pub fn literal_type(text: &str) -> Option<&'static str> {
    let t = text.trim();
    let first = t.chars().next()?;
    if matches!(first, '"' | '\'' | '`') || t.starts_with("@\"") || t.starts_with("$\"") || t.starts_with("$@\"") || t.starts_with("@$\"") {
        // `'a'` is a character in C#; still text for the reader.
        return Some("text");
    }
    if first.is_ascii_digit() || (first == '.' && t.len() > 1) {
        return Some("number");
    }
    if t == "true" || t == "false" {
        return Some("bool");
    }
    None
}

fn is_missing_literal(e: &Expr) -> bool {
    e.concept == Concept::Literal && matches!(e.label.as_str(), "null" | "undefined" | "default")
}

fn is_text(e: &Expr) -> bool {
    e.ty.as_deref() == Some("text")
        || (e.concept == Concept::Compose && e.flavors.iter().any(|f| f == "interpolation"))
        || e.op.as_deref() == Some("text.concat")
}

/// Fills `ty` on literals and interpolated texts below `e` that do not have one yet.
pub fn type_leaves(e: &mut Expr) {
    for c in &mut e.children {
        type_leaves(c);
    }
    if e.ty.is_none() {
        e.ty = match e.concept {
            Concept::Literal => literal_type(&e.label).map(str::to_owned),
            Concept::Compose if e.flavors.iter().any(|f| f == "interpolation") => Some("text".into()),
            _ => None,
        };
    }
}

/// Marks `e` (a `BinaryOp` or `UnaryOp` lowered from `op`) as a shared operation:
/// `Expr::op` names the catalog entry, pure ones become [`Concept::Operation`].
/// Unknown operators are left as they are.
pub fn operator(mut e: Expr, op: &str, equality: Equality) -> Expr {
    for c in &mut e.children {
        type_leaves(c);
    }
    let key = match e.children.len() {
        1 => unary_key(op),
        _ => binary_key(op, equality),
    };
    let Some(mut key) = key else { return e };
    // `x == null`, `x !== undefined`: one-input checks.
    if matches!(key, "equal" | "not_equal" | "loose_equal" | "loose_not_equal")
        && e.children.len() == 2
        && let Some(i) = e.children.iter().position(is_missing_literal)
    {
        e.children.remove(i);
        key = if key.contains("not") { "is_present" } else { "is_missing" };
    }
    if key == "add" && e.children.iter().any(is_text) {
        key = "text.concat";
    }
    e.ty = result_type(key, &e.children).map(str::to_owned);
    e.op = Some(key.to_owned());
    if !is_impure(key) {
        e.concept = Concept::Operation;
    }
    e
}

/// `total += x`: stays an assignment, `Expr::op` names the operation it applies.
pub fn compound_assign(mut e: Expr, op: &str, equality: Equality) -> Expr {
    if let Some(base) = op.strip_suffix('=')
        && !base.is_empty()
        && let Some(key) = binary_key(base, equality)
    {
        for c in &mut e.children {
            type_leaves(c);
        }
        let key = if key == "add" && e.children.iter().any(is_text) { "text.concat" } else { key };
        e.op = Some(key.to_owned());
    }
    e
}

/// Parameter names from a parameter list as written: `(a, b)`, `x`, `(int x, string y)`,
/// `({ id }: Row, n = 1)`.
pub fn param_names(text: &str) -> Vec<String> {
    let inner = text.trim().trim_start_matches('(').trim_end_matches(')');
    split_top_level(inner)
        .into_iter()
        .filter_map(|p| {
            let p = p.trim();
            if p.is_empty() {
                return None;
            }
            // Destructuring keeps its pattern; types and defaults are dropped.
            if p.starts_with('{') || p.starts_with('[') {
                let end = matching_close(p).unwrap_or(p.len());
                return Some(p[..end].to_owned());
            }
            let p = p.split(['=', ':']).next().unwrap_or(p).trim();
            let p = p.trim_start_matches("...").trim_end_matches('?');
            // C#: `int x`, `ref int x`, `params int[] xs` → the last word.
            let name = p.split_whitespace().last().unwrap_or(p);
            (!name.is_empty()).then(|| name.to_owned())
        })
        .collect()
}

fn split_top_level(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

fn matching_close(text: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_ir::Span;

    fn leaf(concept: Concept, label: &str) -> Expr {
        Expr {
            concept,
            label: label.into(),
            span: Span::default(),
            flavors: Vec::new(),
            children: Vec::new(),
            symbol: None,
            target: None,
            op: None,
            ty: None,
            params: Vec::new(),
        }
    }

    fn bin(label: &str, a: Expr, b: Expr) -> Expr {
        let mut e = leaf(Concept::BinaryOp, label);
        e.children = vec![a, b];
        e
    }

    #[test]
    fn comparison_is_a_boolean_operation() {
        let e = operator(bin(">=", leaf(Concept::Var, "a"), leaf(Concept::Literal, "10")), ">=", Equality::Strict);
        assert_eq!(e.concept, Concept::Operation);
        assert_eq!(e.op.as_deref(), Some("greater_or_equal"));
        assert_eq!(e.ty.as_deref(), Some("bool"));
        assert_eq!(e.children[1].ty.as_deref(), Some("number"));
    }

    #[test]
    fn null_checks_and_text_concat() {
        let e = operator(bin("==", leaf(Concept::Var, "x"), leaf(Concept::Literal, "null")), "==", Equality::Loose);
        assert_eq!(e.op.as_deref(), Some("is_missing"));
        assert_eq!(e.children.len(), 1);
        let e = operator(bin("+", leaf(Concept::Literal, "\"Hi \""), leaf(Concept::Var, "name")), "+", Equality::Strict);
        assert_eq!(e.op.as_deref(), Some("text.concat"));
        let e = operator(bin("==", leaf(Concept::Var, "a"), leaf(Concept::Var, "b")), "==", Equality::Loose);
        assert_eq!(e.op.as_deref(), Some("loose_equal"));
    }

    #[test]
    fn increment_stays_on_the_chain() {
        let mut e = leaf(Concept::UnaryOp, "++");
        e.children = vec![leaf(Concept::Var, "i")];
        let e = operator(e, "++", Equality::Strict);
        assert_eq!(e.concept, Concept::UnaryOp);
        assert_eq!(e.op.as_deref(), Some("increment"));
    }

    #[test]
    fn parameter_names() {
        assert_eq!(param_names("(a, b)"), ["a", "b"]);
        assert_eq!(param_names("x"), ["x"]);
        assert_eq!(param_names("(int x, Dictionary<string, int> map)"), ["x", "map"]);
        assert_eq!(param_names("({ id, name }: Row, n = 1, ...rest)"), ["{ id, name }", "n", "rest"]);
        assert_eq!(param_names("()"), Vec::<String>::new());
    }
}
