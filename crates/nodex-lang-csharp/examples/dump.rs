//! Prints the tree-sitter CST of a C# file with field names. Usage: cargo run --example dump -- <file.cs>
fn main() {
    let path = std::env::args().nth(1).expect("path");
    let src = std::fs::read_to_string(path).unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_c_sharp::LANGUAGE.into()).unwrap();
    let tree = parser.parse(&src, None).unwrap();
    let mut cursor = tree.walk();
    walk(&mut cursor, &src, 0);
}

fn walk(c: &mut tree_sitter::TreeCursor, src: &str, depth: usize) {
    loop {
        let n = c.node();
        if n.is_named() {
            let field = c.field_name().map(|f| format!("{f}: ")).unwrap_or_default();
            let text = if n.child_count() == 0 { format!(" `{}`", &src[n.byte_range()]) } else { String::new() };
            println!("{}{}{}{}", "  ".repeat(depth), field, n.kind(), text);
        }
        if c.goto_first_child() {
            walk(c, src, depth + 1);
            c.goto_parent();
        }
        if !c.goto_next_sibling() {
            break;
        }
    }
}
