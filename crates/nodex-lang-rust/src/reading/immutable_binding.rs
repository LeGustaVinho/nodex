use super::*;

impl Reader<'_, '_> {
    pub(super) fn explain_immutable_binding(&mut self, node: Node<'_>, pattern: Node<'_>, value: Option<Node<'_>>) {
        if node.kind() != "let_declaration"
            || pattern.kind() != "identifier"
            || value.is_none()
            || children(node).iter().any(|child| child.kind() == "mutable_specifier")
        {
            return;
        }
        let name = self.src.norm(pattern);
        let inferred = if node.child_by_field_name("type").is_none() {
            " Rust infers its type from the initializer and surrounding context."
        } else {
            ""
        };
        self.push(self.src.span(node), self.src.span(pattern), false, detail(
            "Introduce an immutable binding",
            format!("Evaluate the expression on the right and bind its result to {name}. This binding cannot be reassigned; this does not necessarily prevent changing data through types that allow interior mutation.{inferred}"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::kit::LineIndex;

    #[test]
    fn immutable_let_explains_binding_and_inference_without_describing_mut_as_immutable() {
        let text = "fn f() { let x = 10; let mut y = 11; let z: i32 = 12; }";
        let tree = crate::parse(text).unwrap();
        let index = LineIndex::new(text);
        let src = Src { text, file: "let.rs", index: &index };
        let (facts, _, _) = collect(src, tree.root_node(), &Evidence::default());
        let details: Vec<_> = facts.iter().filter_map(|fact| match &fact.content {
            ReadingContent::Detail { label, text, .. } if label == "Introduce an immutable binding" => Some(text.as_str()),
            _ => None,
        }).collect();
        assert_eq!(details.len(), 2);
        assert!(details[0].contains("bind its result to x"));
        assert!(details[0].contains("infers its type"));
        assert!(details[0].contains("cannot be reassigned"));
        assert!(details[1].contains("bind its result to z"));
        assert!(!details[1].contains("infers its type"));
    }
}

