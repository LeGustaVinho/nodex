//! Items of every kind.

pub const LIMIT: u32 = 10;
pub static GREETING: &str = "hello";
static mut COUNTER: u32 = 0;

pub type Callback = fn(u32) -> u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Level {
    Low = 1,
    High = 2,
}

pub(crate) struct Internal;

#[must_use]
#[inline]
pub fn double(x: u32) -> u32 {
    x * 2
}

pub fn outer(x: u32) -> u32 {
    fn helper(y: u32) -> u32 {
        y + 1
    }
    struct Local {
        value: u32,
    }
    let local = Local { value: helper(x) };
    local.value
}

unsafe extern "C" {
    fn abs(input: i32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles() {
        assert_eq!(double(2), 4);
    }
}
