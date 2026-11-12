use std::ops::Sub;

pub struct Point {
    pub x: f64,
    pub y: f64,
}

pub fn coordinate_delta(a: Point, b: Point) -> (i64, i64) {
    let dx = a.x - b.x;
    (dx as i64, (a.y - b.y) as i64)
}

pub fn arithmetic(a: i16, b: i16, c: &f64, d: &f64) -> (i64, i64, i64, i64, i64) {
    (
        (a + b) as i64,
        (a * b) as i64,
        (a / b) as i64,
        (a % b) as i64,
        (c - d) as i64,
    )
}

pub fn borrowed_delta(a: f64, b: f64) -> f64 {
    let ref borrowed = a - b;
    *borrowed
}

#[derive(Clone, Copy)]
pub struct Right;

impl Sub<Right> for f64 {
    type Output = i16;

    fn sub(self, _: Right) -> Self::Output {
        1
    }
}

pub fn custom_delta(left: f64, right: Right) -> (i64, i64) {
    let dx = left - right;
    (dx as i64, (left - right) as i64)
}
