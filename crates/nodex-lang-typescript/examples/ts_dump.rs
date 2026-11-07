//! Prints the tree-sitter tree of a TypeScript/JavaScript file (debug aid).
//! Usage: cargo run -p nodex-lang-typescript --example ts_dump -- <file>
fn main() {
    let path = std::env::args().nth(1).expect("file");
    let text = std::fs::read_to_string(&path).expect("read");
    let tsx = !(path.ends_with(".ts") || path.ends_with(".mts") || path.ends_with(".cts"));
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&nodex_lang_typescript::grammar(tsx)).unwrap();
    let tree = parser.parse(&text, None).unwrap();
    let mut cursor = tree.walk();
    let mut depth = 0;
    loop {
        let n = cursor.node();
        if n.is_named() {
            let field = cursor.field_name().map(|f| format!("{f}: ")).unwrap_or_default();
            let snippet: String = text[n.byte_range()].split_whitespace().collect::<Vec<_>>().join(" ").chars().take(50).collect();
            println!("{}{}{}{}  `{}`", "  ".repeat(depth), field, n.kind(), if n.is_error() { " !!" } else { "" }, snippet);
        }
        if cursor.goto_first_child() { depth += 1; continue; }
        loop {
            if cursor.goto_next_sibling() { break; }
            if !cursor.goto_parent() { return; }
            depth -= 1;
        }
    }
}
