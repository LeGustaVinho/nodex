//! Explain value parameters on generic struct declarations.
use nodex_ir::reading::ContractItem;
use nodex_lang_api::syntax::{named_children, Src};
use tree_sitter::Node;

pub(crate) fn contract(src: Src<'_>, node: Node<'_>) -> Vec<ContractItem> {
    if node.kind() != "struct_item" { return Vec::new(); }
    let Some(name) = node.child_by_field_name("name") else { return Vec::new(); };
    let Some(parameters) = node.child_by_field_name("type_parameters") else { return Vec::new(); };
    named_children(parameters).into_iter().filter_map(|parameter| {
        if parameter.kind() != "const_parameter" { return None; }
        let parameter_name = parameter.child_by_field_name("name")?;
        let ty = parameter.child_by_field_name("type")?;
        Some(ContractItem {
            group: "Const generic parameter".into(),
            text: format!("{} is a generic struct parameterized by a constant value. `const {}: {}` declares the value parameter {} and its type; it does not declare a field. A use such as `{}<4>` supplies a compile-time value where the parameter type permits it. Different argument values denote different instantiations of the struct type. The parameter can be used in supported compile-time positions, for example as an array length; Rust restricts which constant expressions are allowed. Any written default can be used where Rust permits omission of the argument.", src.norm(name), src.norm(parameter_name), src.norm(ty), src.norm(parameter_name), src.norm(name)),
            span: src.span(parameter),
            type_info: None,
        })
    }).collect()
}
