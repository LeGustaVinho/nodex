//! Explain `$crate::name` in a declarative macro transcriber.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, declaration: Node<'_>) -> Vec<ContractItem> {
    if declaration.kind() != "macro_definition" { return Vec::new(); }
    let mut facts = Vec::new();
    for rule in named_children(declaration).into_iter().filter(|item| item.kind() == "macro_rule" && !item.has_error()) {
        let Some(template) = rule.child_by_field_name("right") else { continue; };
        let mut pending = vec![template];
        while let Some(node) = pending.pop() {
            // String and character literal contents are data, not macro token paths.
            if node.is_extra() || matches!(node.kind(), "string_literal" | "raw_string_literal" | "char_literal") { continue; }
            let parts: Vec<_> = children(node).into_iter().filter(|part| !part.is_extra()).collect();
            for window in parts.windows(3) {
                let (root, separator, name) = (window[0], window[1], window[2]);
                if root.kind() != "metavariable" || src.text(root) != "$crate" || separator.kind() != "::" || !matches!(name.kind(), "identifier" | "type_identifier") { continue; }
                facts.push(ContractItem {
                    group: "Macro defining-crate path".into(),
                    text: format!("In this macro expansion template, $crate::{} starts at the root of the crate that defines this macro, even when a different crate invokes or imports it. $crate is a special macro metavariable; :: continues the path from that defining crate. This helps the expansion refer to the defining crate without requiring the caller to import its crate name. It does not bypass visibility: items used by an expansion must still be accessible from the invocation context. The macro definition itself does not call or resolve the item; the generated code is checked in context.", src.norm(name)),
                    span: src.span(root),
                    type_info: None,
                });
            }
            pending.extend(named_children(node).into_iter().filter(|child| !child.is_extra()).rev());
        }
    }
    facts
}
