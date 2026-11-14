use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

/// Describe simple grouped imports belonging directly to this module.
pub(crate) fn explain(src: Src, module: Node<'_>) -> Vec<ContractItem> {
    let container = match module.kind() {
        "source_file" => module,
        "mod_item" => match module.child_by_field_name("body") {
            Some(body) => body,
            None => return Vec::new(),
        },
        _ => return Vec::new(),
    };
    named_children(container).into_iter().filter_map(|item| {
        if item.kind() != "use_declaration" { return None; }
        let argument = item.child_by_field_name("argument")?;
        if argument.kind() != "scoped_use_list" { return None; }
        let prefix = argument.child_by_field_name("path")?;
        let list = argument.child_by_field_name("list")?;
        let members = named_children(list);
        if members.is_empty() || !members.iter().all(|member| member.kind() == "identifier") {
            return None;
        }
        let prefix = src.norm(prefix);
        let names = members.iter().map(|member| src.norm(*member)).collect::<Vec<_>>();
        let paths = names.iter().map(|name| format!("{prefix}::{name}")).collect::<Vec<_>>();
        Some(ContractItem {
            group: "Grouped import".into(),
            text: format!(
                "Bring {} into this module's scope. The braces share the prefix {prefix}; this is equivalent to separate imports of {}. These names refer to existing items; the statement does not define them.",
                names.join(", "), paths.join(" and "),
            ),
            span: src.span(item),
            type_info: None,
        })
    }).collect()
}
