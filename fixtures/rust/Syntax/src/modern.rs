//! Recent Rust: 1.65-1.85 and edition 2024.

use std::collections::HashMap;
use std::fmt::Debug;

/// Raw identifiers, raw borrows, C strings, inline const, exclusive range patterns.
pub fn modern(values: &[u8], r#type: &str) -> usize {
    let r#match = r#type.len();
    let raw = &raw const r#match;
    let c_text = c"hello";
    let bytes = b"bytes";
    let raw_bytes = br"raw\bytes";
    let zero = const { 1 + 1 };
    let kind = match values.len() {
        0..5 => "small",
        5.. => "large",
    };
    let label = 'block: {
        if values.is_empty() {
            break 'block "empty";
        }
        "full"
    };
    let first = values.first().copied().unwrap_or_default();
    let doubled: Vec<u8> = values.iter().map(|v| v * 2).collect::<Vec<_>>();
    let typed = Vec::<u32>::with_capacity(4);
    let max = 1u32.max(2);
    let nested = Some(Some(3)).flatten();
    let joined = [r#match, zero, kind.len(), label.len(), first as usize, doubled.len(), typed.len(), max as usize].iter().sum::<usize>();
    unsafe { *raw + joined + nested.unwrap_or(0) + c_text.count_bytes() + bytes.len() + raw_bytes.len() }
}

/// Async closures (1.85) and async functions in traits (1.75).
pub trait Fetch {
    async fn fetch(&self, id: u32) -> u32;
    fn names(&self) -> impl Iterator<Item = String>;
}

pub async fn async_closures(items: Vec<u32>) -> u32 {
    let add = async |x: u32| x + 1;
    let mut total = 0;
    for item in items {
        total += add(item).await;
    }
    total
}

/// Precise capturing (2024) and return-position impl Trait.
pub fn captures<'a>(text: &'a str) -> impl Iterator<Item = char> + use<'a> {
    text.chars().filter(|c| c.is_alphabetic())
}

/// Generic associated types, higher-ranked bounds, `?Sized` and the never type.
pub fn apply<F>(f: F, text: &str) -> usize
where
    F: for<'b> Fn(&'b str) -> usize,
{
    f(text)
}

pub fn show<T: ?Sized + Debug>(value: &T) -> String {
    format!("{value:?}")
}

pub fn never() -> ! {
    loop {
        std::thread::park();
    }
}

/// Labelled loops returning values and `while let` over an iterator.
pub fn search(grid: &[Vec<i32>], target: i32) -> Option<(usize, usize)> {
    let found = 'outer: loop {
        for (r, row) in grid.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                if *cell == target {
                    break 'outer Some((r, c));
                }
            }
        }
        break None;
    };
    let mut it = grid.iter();
    while let Some(row) = it.next() {
        if row.is_empty() {
            continue;
        }
    }
    found
}

/// Option and Result combinators with `?`.
pub fn lookup(map: &HashMap<String, Vec<u32>>, key: &str) -> Option<u32> {
    let list = map.get(key)?;
    let first = list.first()?;
    Some(*first + 1)
}

pub fn parse_all(texts: &[&str]) -> Result<Vec<i64>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    for t in texts {
        out.push(t.parse::<i64>()?);
    }
    if out.is_empty() {
        return Err("nothing parsed".into());
    }
    Ok(out)
}

#[unsafe(no_mangle)]
pub extern "C" fn exported(x: i32) -> i32 {
    x + 1
}

unsafe extern "C" {
    pub safe fn abs(x: i32) -> i32;
    pub unsafe fn strlen(p: *const std::ffi::c_char) -> usize;
}
