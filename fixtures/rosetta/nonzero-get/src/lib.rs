use std::collections::HashMap;
use std::num::{NonZero, NonZeroI32 as SignedCount};

const WORKERS: usize = 8;

pub fn workers() -> usize {
    std::thread::available_parallelism().map_or(1, |cores| cores.get().min(WORKERS))
}

pub fn signed(count: SignedCount) -> i32 {
    SignedCount::get(count)
}

pub fn explicit(count: NonZero<u32>) -> u32 {
    count.get()
}

pub fn lookup(table: &HashMap<u32, String>, key: u32) -> Option<&String> {
    table.get(&key)
}

mod local {
    struct NonZero;

    impl NonZero {
        fn get(self) -> i32 { 0 }
    }

    pub fn local_zero() -> i32 { NonZero.get() }
}
