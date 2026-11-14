//! Wording for a bounded inclusive range expression.

/// Does not assume an element type, iteration, or name resolution.
pub(crate) fn describe(start: &str, end: &str) -> (&'static str, String) {
    ("Range includes the end", format!("{}..={} creates a range with {} as its start bound and {} as its end bound. Both bounds are included. This creates a range value; the surrounding code decides how it is used, for example in a loop or a membership check. Its element type and permitted operations depend on context.", start, end, start, end))
}
