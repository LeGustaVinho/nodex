pub fn lazy(enabled: bool, probe: Option<i32>) -> Option<Option<i32>> {
    enabled.then(|| Some(probe? + 1))
}

#[allow(unused_parens)]
pub fn lazy_ufcs(enabled: bool, probe: Option<i32>) -> Option<Option<i32>> {
    bool::then(enabled, (|| Some(probe? + 1)))
}

pub fn eager(probe: Option<i32>) -> Option<i32> {
    bool::then_some(false, probe?)
}

struct Local;

impl Local {
    fn then<T>(&self, callback: impl FnOnce() -> T) -> Option<T> {
        Some(callback())
    }

    fn then_some<T>(&self, _: T) -> Option<T> {
        None
    }
}

pub fn local_then(probe: Option<i32>) -> Option<Option<i32>> {
    Local.then(|| Some(probe? + 1))
}

pub fn local_then_some(probe: Option<i32>) -> Option<i32> {
    Local.then_some(probe?)
}

pub fn assign_boolean(values: &[bool], probe: impl FnOnce() -> bool) -> (bool, bool) {
    let mut lost = true;
    lost |= values.iter().copied().any(std::convert::identity);
    let mut same = false;
    same &= probe();
    (lost, same)
}

pub fn assign_with_early_exit(mut lost: bool, update: Option<bool>) -> Option<bool> {
    lost |= update?;
    Some(lost)
}

pub fn assign_integer(mut mask: u32, flags: u32) -> u32 {
    mask |= flags;
    mask &= flags;
    mask
}

pub fn assign_by_reference(mut value: bool, other: &bool) -> bool {
    value |= other;
    value &= other;
    value
}

struct Switch;

impl std::ops::BitOrAssign<Switch> for bool {
    fn bitor_assign(&mut self, _: Switch) {
        *self = !*self;
    }
}

pub fn assign_custom(mut value: bool) -> bool {
    value |= Switch;
    value
}

pub fn lazy_operators(left: bool, right: impl Fn() -> bool) -> (bool, bool) {
    (left || right(), left && right())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compound_assignment_still_evaluates_the_right_side() {
        let mut calls = 0;
        assert_eq!(
            assign_boolean(&[false], || {
                calls += 1;
                true
            }),
            (true, false)
        );
        assert_eq!(calls, 1);
        assert_eq!(assign_with_early_exit(true, None), None);
    }
}
