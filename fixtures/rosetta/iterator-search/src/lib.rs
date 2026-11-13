pub fn lookup(entries: Vec<(String, u32)>) -> Option<u32> {
    let (_, value) = entries.into_iter().find(|(key, _)| key == "id")?;
    Some(value)
}

pub fn scan(items: &[u8]) -> Option<&u8> {
    items.iter().find(|&&item| {
        if item > 2 {
            return true;
        }
        let local = || {
            return true;
        };
        std::hint::black_box(local());
        false
    })
}

pub fn scan_mut(items: &mut [u8]) -> Option<&mut u8> {
    items.iter_mut().find(|item| **item > 2)
}

pub fn locate(items: &[u8]) -> Option<usize> {
    items.iter().position(|&item| {
        if item == 0 {
            return false;
        }
        true
    })
}

pub fn after_advance() -> (Option<usize>, Option<(u8, u8)>) {
    let mut pairs = [10, 20, 30].into_iter().zip([1, 2, 3]);
    pairs.next();
    let index = Iterator::position(&mut pairs, |pair| pair == (20, 2));
    (index, pairs.next())
}

pub fn reversed(items: &[u8]) -> Option<&u8> {
    items.iter().rev().find(|&&item| item > 2)
}

pub fn chained<'a>(left: &'a [u8], right: &'a [u8]) -> Option<&'a u8> {
    left.iter().chain(right).find(|&&item| item > 2)
}

pub fn text(input: &str) -> Option<usize> {
    input.find('!')
}

pub struct Search;
impl Search {
    fn find(&mut self, mut test: impl FnMut(&u8) -> bool) -> Option<u8> {
        test(&0).then_some(0)
    }
}

pub fn custom(input: &mut Search) -> Option<u8> {
    input.find(|_| {
        return true;
    })
}

pub fn any_active(items: &[u8]) -> bool {
    items.iter().any(|&item| {
        if item > 2 {
            return true;
        }
        false
    })
}

pub fn all_pairs(left: &[u8], right: &[u8]) -> bool {
    let mut pairs = left.iter().zip(right);
    Iterator::all(&mut pairs, |(left, right)| {
        if left != right {
            return false;
        }
        true
    })
}

pub fn ascii(input: &str) -> bool {
    input.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

pub fn has_separator(input: &str) -> bool {
    input.bytes().any(|byte| byte == b':')
}

pub fn all_increased(items: &mut [u8]) -> bool {
    items.iter_mut().all(|item| {
        *item = item.saturating_add(1);
        *item > 1
    })
}

pub fn pending(items: &[u8]) -> bool {
    let mut items = items.iter().copied().peekable();
    items.peek().is_none() || items.any(|item| item > 2)
}

pub fn after_decisive() -> ((bool, Option<u8>), (bool, Option<u8>)) {
    let mut any = [1, 2, 3].into_iter();
    let matched = any.any(|item| item == 2);
    let mut all = [1, 2, 3].into_iter();
    let passed = all.all(|item| item < 2);
    ((matched, any.next()), (passed, all.next()))
}

pub fn empty_results() -> (bool, bool) {
    (
        std::iter::empty::<u8>().any(|_| panic!("empty iterator")),
        std::iter::empty::<u8>().all(|_| panic!("empty iterator")),
    )
}

pub struct CustomIter;
impl Iterator for CustomIter {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        None
    }

    fn all<F>(&mut self, mut test: F) -> bool
    where
        F: FnMut(u8) -> bool,
    {
        test(42)
    }
}

pub fn custom_all(mut input: CustomIter) -> bool {
    input.all(|_| {
        return false;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truth_and_progress() {
        assert_eq!(empty_results(), (false, true));
        assert_eq!(after_decisive(), ((true, Some(3)), (false, Some(3))));
        assert!(!all_pairs(&[1, 2], &[1, 3]));
    }
}
