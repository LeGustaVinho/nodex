use rayon::prelude::*;

pub fn compare_ids(a: &str, b: &str) -> std::cmp::Ordering {
    let (x, y) = (a.as_bytes(), b.as_bytes());
    match x.iter().zip(y).position(|(p, q)| p != q) {
        Some(at) => x[at].cmp(&y[at]),
        None => x.len().cmp(&y.len()),
    }
}

pub fn same_baselines(current: &[i32], known: &[i32]) -> bool {
    current.len() == known.len() && current.iter().zip(known).all(|(a, b)| a == b)
}

pub fn collect_pairs<'a, 'b>(left: &'a [i32], right: &'b [i32]) -> Vec<(&'a i32, &'b i32)> {
    left.iter().zip(right).collect()
}

pub fn qualified(left: impl Iterator<Item = i32>, right: [i32; 2]) -> Option<(i32, i32)> {
    Iterator::zip(left, right).next()
}

pub fn option_zip(left: Option<i32>, right: Option<i32>) -> Option<(i32, i32)> {
    left.zip(right)
}

pub fn parallel_zip(left: &[i32], right: &[i32]) -> i32 {
    left.par_iter().zip(right).map(|(a, b)| a + b).sum()
}

pub struct Custom;

impl Custom {
    pub fn zip(&self, other: i32) -> i32 {
        other
    }
}

pub fn custom_zip(custom: Custom) -> i32 {
    custom.zip(10)
}
