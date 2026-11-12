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
