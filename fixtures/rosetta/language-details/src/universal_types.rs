use std::collections::VecDeque;

pub struct Vec<T>(T);

pub fn rust_collections() {
    let items = vec![1_i32];
    let own = Vec(2_u64);
    let nested = crate::standard_nested();
    let nested_vectors = vec![vec![1_i32]];
    let copied_type = &nested;
    let mut fifo = VecDeque::<i32>::new();
    fifo.push_back(1);
    fifo.pop_front();
    let mut defaulted = VecDeque::default();
    defaulted.push_back(1_i32);
    defaulted.pop_front();
    let mut lifo = VecDeque::<i32>::new();
    lifo.push_front(1);
    lifo.pop_front();
    let mut mixed = VecDeque::<i32>::new();
    mixed.push_back(1);
    mixed.pop_back();
    mixed.pop_front();
    let (deconstructed, marker) = (vec![2_i32], 3_u64);
    {
        macro_rules! vec { () => { Vec(7_u64) } }
        let custom_elements = std::vec![vec![]];
        let _ = custom_elements;
    }
    let _ = (items, own, copied_type, deconstructed, marker, nested_vectors);
}

fn identity<T>(value: T) -> T { value }
fn first<T>(pair: (T, bool)) -> T { pair.0 }

pub struct Wrapper<T>(T);

impl<T: Copy> Wrapper<T> {
    fn get(&self) -> T { self.0 }
}

pub fn rust_generic_expression(value: &Wrapper<u64>) -> (i32, u64, i32, u64, u64, &'static std::ffi::CStr) {
    (identity(1_i32), identity::<u64>(2), first((3_i32, true)), identity(4), value.get(), identity(c"example"))
}
