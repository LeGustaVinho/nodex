use std::fmt::{self, Debug, Display};
use std::ops::Add;

/// A trait with a generic method, an associated type and a default body.
pub trait Container {
    type Item;
    const CAPACITY: usize = 8;

    fn get(&self, index: usize) -> Option<&Self::Item>;
    fn first(&self) -> Option<&Self::Item> {
        self.get(0)
    }
    fn map_all<F>(&self, f: F) -> Vec<String>
    where
        F: Fn(&Self::Item) -> String;
}

/// A generic associated type.
pub trait Lending {
    type Item<'a>
    where
        Self: 'a;
    fn lend(&mut self) -> Option<Self::Item<'_>>;
}

#[derive(Debug, Default)]
pub struct Stack<T> {
    items: Vec<T>,
}

impl<T: Debug> Container for Stack<T> {
    type Item = T;

    fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    fn map_all<F>(&self, f: F) -> Vec<String>
    where
        F: Fn(&T) -> String,
    {
        self.items.iter().map(f).collect()
    }
}

impl<T> Stack<T> {
    pub fn new() -> Self {
        Stack { items: Vec::new() }
    }

    pub fn push(&mut self, item: T) -> &mut Self {
        self.items.push(item);
        self
    }
}

impl<T: Display> Display for Stack<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for item in &self.items {
            write!(f, "{item} ")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Money(pub i64);

impl Add for Money {
    type Output = Money;
    fn add(self, other: Money) -> Money {
        Money(self.0 + other.0)
    }
}

impl From<i64> for Money {
    fn from(v: i64) -> Self {
        Money(v)
    }
}

pub struct Buffer<const N: usize> {
    data: [u8; N],
}

impl<const N: usize> Buffer<N> {
    pub const fn size() -> usize {
        N
    }
}

pub fn describe(items: &impl Container<Item = i32>) -> String {
    format!("{:?}", items.first())
}

pub fn largest<T: PartialOrd + Copy>(items: &[T]) -> T {
    let mut best = items[0];
    for &item in items {
        if item > best {
            best = item;
        }
    }
    best
}

pub union IntOrFloat {
    pub i: u32,
    pub f: f32,
}
