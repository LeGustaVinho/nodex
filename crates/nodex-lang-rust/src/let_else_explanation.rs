//! Text for the two outcomes of a Rust `let ... else` statement.

/// Returns the input and explanations for the matching and non-matching paths.
/// The pattern is deliberately kept generic: even `Some` can be a user-defined
/// name, so syntax alone cannot prove it refers to the standard `Option` type.
pub(crate) fn explain(value: &str, pattern: &str) -> (String, String, String) {
    (
        format!("Evaluate {value} once, then check whether its result matches the pattern {pattern}."),
        format!("The result has the shape required by {pattern}. Extract the parts named by the pattern; those names are available in the surrounding scope after this statement. Skip the else block and continue below."),
        "The result does not match the pattern, so run the else block. No names from the pattern are available there. The block must leave this path or never finish, for example by return, break, continue, or panic; it cannot continue below the statement.".into(),
    )
}

#[cfg(test)]
mod tests {
    use super::explain;

    #[test]
    fn describes_both_paths_without_assuming_some_is_std_option() {
        let (input, success, mismatch) = explain("value", "Some(x)");
        assert!(input.contains("Evaluate value once"));
        assert!(input.contains("Some(x)"));
        assert!(success.contains("names are available"));
        assert!(success.contains("Skip the else block"));
        assert!(mismatch.contains("No names from the pattern"));
        assert!(mismatch.contains("return"));
        assert!(mismatch.contains("cannot continue below"));
        assert!(!success.contains("Option"));
    }
}
