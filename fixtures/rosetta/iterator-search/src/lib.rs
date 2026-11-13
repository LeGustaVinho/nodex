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
