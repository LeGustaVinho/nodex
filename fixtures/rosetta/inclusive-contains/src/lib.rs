use std::ops::RangeInclusive;

pub fn combat(ids: Option<Vec<u32>>, distinct: impl FnOnce(&Vec<u32>) -> bool) -> Option<Vec<u32>> {
    let Some(ids) = ids.filter(|ids| (2..=64).contains(&ids.len()) && distinct(ids)) else {
        return None;
    };
    Some(ids)
}

pub fn comparable(range: &RangeInclusive<f64>, item: &f64) -> bool {
    RangeInclusive::contains(range, item)
}

pub fn exhausted(mut range: RangeInclusive<u8>, item: u8) -> bool {
    for _ in range.by_ref() {}
    range.contains(&item)
}

pub fn exclusive(item: u32) -> bool { (2..64).contains(&item) }
pub fn text_contains(text: &str) -> bool { text.contains('-') }
pub fn slice_contains(slice: &[u32], item: u32) -> bool { slice.contains(&item) }

mod local {
    struct RangeInclusive;

    impl RangeInclusive {
        fn contains(&self, _: &u32) -> bool { true }
    }

    pub fn local_contains() -> bool { RangeInclusive.contains(&0) }
}
