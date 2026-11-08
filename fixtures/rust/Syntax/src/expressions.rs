use std::rc::Rc;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

pub struct Pair(pub i32, pub i32);

/// Operators, literals, references and conversions.
pub fn expressions(a: i32, b: i32, name: &str, flags: u8) -> String {
    let sum = a + b * 2 - (a % 3) / 1;
    let power = 2_i64.pow(3);
    let bits = (flags & 0b1010) | (flags ^ 1) << 2 >> 1;
    let not_bits = !flags;
    let logic = a > b && !(a == b) || a <= 0 && b >= 0;
    let negative = -a;
    let float = 1.5_f64 * 2.0;
    let ch = 'x';
    let raw = r"raw\text";
    let byte = b'a';
    let as_float = a as f64 / 3.0;
    let range = 0..=10;
    let tuple = (a, name, 3.0);
    let (left, right) = (tuple.0, tuple.2);
    let array = [1, 2, 3];
    let zeros = [0u8; 4];
    let indexed = array[1];
    let slice = &array[1..];
    let point = Point { x: 1.0, y: 2.0 };
    let moved = Point { y: 5.0, ..point };
    let pair = Pair(1, 2);
    let max = a.max(b);
    let abs = negative.abs();
    let upper = name.to_uppercase();
    let trimmed = name.trim();
    let len = name.len();
    let parsed: i32 = "12".parse().unwrap_or_default();
    let text = a.to_string();
    let joined = format!("{name}: {sum} {}", point.x);
    let shared = Rc::new(RefCell::new(vec![1]));
    shared.borrow_mut().push(2);
    let cloned = Rc::clone(&shared);
    let mut owned = String::new();
    owned.push_str(name);
    owned += "!";
    let mutable = &mut owned;
    mutable.push('?');
    let reference = &owned;
    let deref = *reference == "x";
    let optional = Some(a).map(|v| v + 1).unwrap_or(0);
    let missing = optional == 0 || name.is_empty();
    let block = {
        let inner = a * 2;
        inner + 1
    };
    let choice = if a > b { "a" } else { "b" };
    let matched = match a {
        1 => "one",
        _ => "many",
    };
    let looped = loop {
        break 5;
    };
    let neg = -1;
    let unit = ();
    joined
}

impl Point {
    pub const ORIGIN: Point = Point { x: 0.0, y: 0.0 };

    pub fn distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    pub fn translate(&mut self, dx: f64) {
        self.x += dx;
    }

    pub fn into_tuple(self) -> (f64, f64) {
        (self.x, self.y)
    }
}
