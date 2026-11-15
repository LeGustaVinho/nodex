//! Explain an `env!("NAME")`-shaped macro invocation without guessing its resolution.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    if node.kind() != "macro_invocation" || node.has_error() { return None; }
    let path = node.child_by_field_name("macro")?;
    let path_text = src.norm(path);
    if path_text.rsplit("::").next()? != "env" { return None; }
    let tokens = named_children(node).into_iter().find(|child| child.kind() == "token_tree")?;
    let open = children(tokens).into_iter().find(|child| !child.is_extra())?;
    if open.kind() != "(" { return None; }
    let first = named_children(tokens).into_iter().find(|child| !child.is_extra())?;
    if !matches!(first.kind(), "string_literal" | "raw_string_literal") || first.has_error() { return None; }
    Some((open, ReadingContent::Detail {
        label: "Compile-time environment lookup".into(),
        text: format!("If {} resolves to Rust's standard `env!` macro, it reads the environment variable named by {} during compilation and embeds its Unicode value as a `&'static str` in the compiled program. A missing or non-Unicode value causes a compile-time error; an optional second argument can customize the missing-variable diagnostic. This is not a runtime environment lookup: changing the environment after compilation does not change the embedded value. The spelling alone does not prove which macro is resolved or what value the build environment contained.", src.norm(path), src.norm(first)),
        definition: None, borrow: false,
    }))
}
