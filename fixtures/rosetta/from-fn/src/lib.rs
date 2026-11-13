use std::{array::from_fn as by_index, iter::from_fn as next_with};

pub fn by_kind() -> [f64; 4] {
    by_index(|kind| kind as f64)
}

pub fn empty_array() -> [u8; 0] {
    std::array::from_fn::<_, 0, _>(|_| panic!("empty array has no positions"))
}

pub fn next_crew() -> impl Iterator<Item = usize> {
    let mut index = 0;
    next_with(move || {
        index += 1;
        (index != 2).then_some(index)
    })
}

pub fn array_callback(seed: Option<u8>) -> [Option<u8>; 1] {
    by_index(|_| Some(seed?))
}

pub fn iterator_callback() -> impl Iterator<Item = u8> {
    next_with(|| Some(Some(7)?))
}

mod local {
    pub mod array {
        pub fn from_fn<T>(mut callback: impl FnMut(usize) -> T) -> T {
            callback(3)
        }
    }

    pub mod iter {
        pub fn from_fn<T>(mut callback: impl FnMut() -> Option<T>) -> Option<T> {
            callback()
        }
    }
}

pub fn shadowed_calls() -> (usize, Option<u8>) {
    (
        local::array::from_fn(|i| i),
        local::iter::from_fn(|| Some(7)),
    )
}
