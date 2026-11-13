use rayon::prelude::*;

fn js_max(left: f64, right: f64) -> f64 {
    left.max(right)
}

pub fn speed_multiplier(values: &[f64]) -> f64 {
    values.iter().map(|value| 1.0 + value / 100.0).fold(1.0, js_max)
}

pub fn travel(bodies: &[(f64, f64)]) -> f64 {
    bodies.iter().map(|(position, start)| (position - start).abs()).filter(|travel| travel.is_finite()).fold(0.0, f64::max)
}

pub fn combat() -> f64 {
    (0..4).fold(0.0, |sum, kind| {
        let adjusted = js_max(0.0, f64::from(kind));
        sum + adjusted
    })
}

pub fn qualified(values: impl Iterator<Item = i32>) -> i32 {
    Iterator::fold(values, 4, |total, value| {
        let doubled = value * 2;
        total + doubled
    })
}

pub fn fallible_accumulator(values: &[Result<u32, ()>]) -> Result<u32, ()> {
    values.iter().fold(Ok(0), |total: Result<u32, ()>, value| {
        let total = total?;
        Ok(total + (*value)?)
    })
}

pub fn owned(values: Vec<i32>) -> i32 {
    values.into_iter().fold(0, |total, value| total + value)
}

pub fn parallel(values: &[i32]) -> i32 {
    values.par_iter().fold(|| 0, |total, value| total + value).sum()
}

pub struct Custom;

impl Iterator for Custom {
    type Item = i32;

    fn next(&mut self) -> Option<i32> { None }

    fn fold<B, F>(self, seed: B, _: F) -> B
    where F: FnMut(B, i32) -> B {
        seed
    }
}

pub fn custom() -> i32 {
    Custom.fold(0, |total, value| {
        let adjusted = value * 2;
        total + adjusted
    })
}

pub fn closure_seed(values: impl Iterator<Item = ()>) -> impl Fn() -> i32 {
    values.fold(|| { let value = 3; value }, |seed, ()| seed)
}
