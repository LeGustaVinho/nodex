use std::cmp::Ordering;

pub fn by_distance(a: &(f64, usize), b: &(f64, usize)) -> Ordering {
    a.0.total_cmp(&b.0).then(a.1.cmp(&b.1))
}

pub fn by_id(a: &(f64, usize), b: &(f64, usize)) -> Ordering {
    a.0.total_cmp(&b.0).then_with(|| {
        let ordering = a.1.cmp(&b.1);
        ordering
    })
}

pub fn qualified(a: f32, b: f32, fallback: impl FnOnce() -> Ordering) -> Ordering {
    Ordering::then_with(f32::total_cmp(&a, &b), fallback)
}

pub struct Local;

impl Local {
    fn total_cmp(self, _: &Self) -> Self { self }
    fn then(self, _: Ordering) -> Self { self }
    fn then_with(self, _: impl FnOnce() -> Ordering) -> Self { self }
}

pub fn homonyms() {
    Local.total_cmp(&Local).then(Ordering::Equal).then_with(|| Ordering::Less);
}

#[test]
fn comparison_and_tie_breakers() {
    let calls = std::cell::Cell::new(0);
    let tie = || { calls.set(calls.get() + 1); Ordering::Greater };
    assert_eq!(Ordering::Less.then(tie()), Ordering::Less);
    assert_eq!(Ordering::Less.then_with(tie), Ordering::Less);
    assert_eq!(calls.get(), 1);
    assert_eq!(Ordering::Equal.then_with(tie), Ordering::Greater);
    assert_eq!(calls.get(), 2);
    assert_eq!(by_distance(&(-0.0, 1), &(0.0, 0)), Ordering::Less);
    assert_eq!(by_id(&(1.0, 5), &(1.0, 2)), Ordering::Greater);
}
