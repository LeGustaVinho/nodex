//! Explain mutable closure parameters and a trailing parameter comma.
use nodex_ir::reading::{ContractItem, ReadingContent};
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Option<(Node<'t>, ReadingContent)> {
    let parameters = if node.kind() == "closure_expression" && !node.has_error() {
        node.child_by_field_name("parameters")?
    } else if node.kind() == "let_declaration" {
        let parts = children(node);
        let error_index = parts.iter().position(|part| part.is_error())?;
        if error_index == 0 || parts[error_index - 1].kind() != "=" || node.child_by_field_name("value")?.kind() != "block" { return None; }
        parts[error_index]
    } else { return None; };
    let tokens: Vec<_> = children(parameters).into_iter().filter(|part| !part.is_extra()).collect();
    if tokens.len() < 4 || tokens.last()?.kind() != "|" || tokens[tokens.len()-2].kind() != "," { return None; }
    let comma = tokens[tokens.len()-2];
    let parameter = named_children(parameters).into_iter().find(|part| {
        if part.kind() != "parameter" { return false; }
        let Some(pattern) = part.child_by_field_name("pattern") else { return false; };
        children(*part).into_iter().any(|child| child.kind() == "mutable_specifier") || pattern.kind() == "mut_pattern"
    })?;
    let pattern = parameter.child_by_field_name("pattern")?;
    let ty = parameter.child_by_field_name("type")?;
    let name = if pattern.kind() == "mut_pattern" { named_children(pattern).into_iter().find(|part| part.kind() == "identifier")? } else if pattern.kind() == "identifier" { pattern } else { return None; };
    let site = if node.kind() == "let_declaration" { node.child_by_field_name("value")? } else { comma };
    Some((site, ReadingContent::Detail {
        label: "Mutable closure parameter and trailing comma".into(),
        text: format!("The vertical bars contain the closure's parameter list. `mut` makes {} a mutable local parameter binding, so the closure body may reassign or modify it; it does not make the caller's binding mutable. `: {}` specifies its expected type, and the comma immediately before the closing `|` is an optional trailing comma, not another parameter. The body runs when the closure is called. In a block body, a final expression without a semicolon supplies the result; for `{{ x += 1; x }}`, the first statement updates x and the final x is returned. Whether an argument is copied or moved depends on its type; the syntax does not establish capture mode or the exact operation selected by `+=`.", src.norm(name), src.norm(ty)),
        definition: None, borrow: false,
    }))
}

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "function_item" { return Vec::new(); }
    let Some(body) = node.child_by_field_name("body") else { return Vec::new(); };
    let mut pending = vec![body];
    let mut result = Vec::new();
    while let Some(part) = pending.pop() {
        if part.is_extra() || part.kind() == "token_tree" || part.kind() == "function_item" { continue; }
        if part.kind() == "let_declaration" {
            if let Some((site, ReadingContent::Detail { text, .. })) = explain(src, part) {
                result.push(ContractItem { group: "Mutable closure parameter and trailing comma".into(), text, span: src.span(site), type_info: None });
            }
        }
        pending.extend(named_children(part));
    }
    result
}
