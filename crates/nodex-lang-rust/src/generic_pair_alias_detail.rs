//! Explain a generic alias for a pair of the same type.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn explain(src: Src<'_>, node: Node<'_>) -> Option<ContractItem> {
    if node.kind() != "type_item" || node.has_error() { return None; }
    let owner = node.parent()?;
    if owner.kind() != "source_file" && !(owner.kind() == "declaration_list" && owner.parent().is_some_and(|part| part.kind() == "mod_item")) { return None; }
    let name = node.child_by_field_name("name")?;
    let parameters = node.child_by_field_name("type_parameters")?;
    let params: Vec<_> = named_children(parameters).into_iter().filter(|part| !part.is_extra()).collect();
    let [param] = params.as_slice() else { return None; };
    if param.kind() != "type_parameter" { return None; }
    let parameter_name = param.child_by_field_name("name")?;
    let ty = node.child_by_field_name("type")?;
    if ty.kind() != "tuple_type" { return None; }
    let fields: Vec<_> = named_children(ty).into_iter().filter(|part| !part.is_extra()).collect();
    let [first, second] = fields.as_slice() else { return None; };
    let generic = src.norm(parameter_name);
    if src.norm(*first) != generic || src.norm(*second) != generic { return None; }
    let name = src.norm(name);
    Some(ContractItem {
        group: "Generic pair type alias".into(),
        text: format!("`type {name}<{generic}> = ({generic}, {generic});` gives a name to a two-element tuple type parameterized by `{generic}`. Each use supplies one concrete type for `{generic}`; that same type occupies both positions, though the two values need not be equal. For example, `{name}<i32>` denotes `(i32, i32)`, whose first and second elements can be accessed with `.0` and `.1`. This is an alias for the existing tuple type, not a distinct wrapper type, constructor, or runtime value. Any written generic requirements must still be checked in context."),
        span: src.span(node), type_info: None,
    })
}
