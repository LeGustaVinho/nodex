//! Explain feature-gated match arms without assuming the build configuration.
use nodex_ir::reading::ReadingContent;
use nodex_lang_api::syntax::{children, named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain<'t>(src: Src<'_>, node: Node<'t>) -> Vec<(Node<'t>, Node<'t>, ReadingContent)> {
    if node.kind() != "match_expression" { return Vec::new(); }
    let Some(anchor) = children(node).into_iter().find(|part| part.kind() == "match") else { return Vec::new(); };
    let Some(body) = node.child_by_field_name("body") else { return Vec::new(); };
    let mut result = Vec::new();
    for arm in named_children(body).into_iter().filter(|part| part.kind() == "match_arm") {
        for item in named_children(arm).into_iter().filter(|part| part.kind() == "attribute_item") {
            let Some(attr) = named_children(item).into_iter().find(|part| part.kind() == "attribute") else { continue; };
            let Some(name) = named_children(attr).into_iter().find(|part| part.kind() == "identifier") else { continue; };
            if src.text(name) != "cfg" { continue; }
            let Some(args) = attr.child_by_field_name("arguments") else { continue; };
            let parts: Vec<_> = children(args).into_iter().filter(|part| !part.is_extra() && !matches!(part.kind(), "(" | ")")).collect();
            let [key, eq, value] = parts.as_slice() else { continue; };
            if src.text(*key) != "feature" || eq.kind() != "=" || value.kind() != "string_literal" { continue; }
            result.push((anchor, item, ReadingContent::Detail {
                label: "Feature-gated match arm".into(),
                text: format!("`#[cfg(feature = {})]` controls whether the following match arm is included in this compilation. If the build configuration has this feature enabled, the arm participates in matching in its written order; otherwise the arm is removed before the match is checked. This is a compile-time condition, not a runtime test for each value. The remaining arms must still cover the possible values; disabling an arm can therefore affect exhaustiveness. The source spelling alone does not establish whether the feature is enabled for this build.", src.text(*value)),
                definition: None, borrow: false,
            }));
        }
    }
    result
}
