use std::collections::HashMap;

/// Every loop and branch form.
pub fn statements(items: &[i32], map: &HashMap<String, i32>) -> i32 {
    let mut total = 0;
    let limit: i32 = 100;

    for (i, item) in items.iter().enumerate() {
        if i == 0 {
            continue;
        } else if *item > limit {
            break;
        } else {
            total += item;
        }
    }

    for n in 0..10 {
        total -= n;
    }

    let mut countdown = 3;
    while countdown > 0 {
        countdown -= 1;
    }

    let mut stack = vec![1, 2, 3];
    while let Some(top) = stack.pop() {
        total += top;
    }

    let found = 'outer: loop {
        for x in items {
            if *x == 7 {
                break 'outer *x;
            }
        }
        break 0;
    };

    'rows: for row in 0..3 {
        for col in 0..3 {
            if col > row {
                continue 'rows;
            }
        }
    }

    match total {
        0 => total = 1,
        1 | 2 => {
            total *= 2;
        }
        n if n < 0 => return -1,
        _ => {}
    }

    if let Some(v) = map.get("key") {
        total += *v;
    }

    let Some(first) = items.first() else {
        return total;
    };

    {
        let scoped = first + 1;
        total += scoped;
    }

    unsafe {
        total += raw_read(&found as *const i32);
    }

    total
}

unsafe fn raw_read(p: *const i32) -> i32 {
    unsafe { *p }
}

/// let-chains (Rust 2024).
pub fn chains(a: Option<i32>, b: Option<i32>) -> i32 {
    if let Some(x) = a
        && let Some(y) = b
        && x < y
    {
        return x + y;
    }
    0
}

/// Errors with ?, early returns and panics.
pub fn errors(text: &str) -> Result<i32, std::num::ParseIntError> {
    let n: i32 = text.trim().parse()?;
    if n < 0 {
        panic!("negative: {n}");
    }
    if n == 0 {
        todo!()
    }
    let doubled = text.parse::<i32>()? * 2;
    assert!(doubled >= 0, "doubled must be positive");
    assert_eq!(doubled % 2, 0);
    Ok(n + doubled)
}

/// A tail match that returns from every arm.
pub fn classify(n: i32) -> &'static str {
    match n {
        i32::MIN..=-1 => "negative",
        0 => "zero",
        _ => "positive",
    }
}

/// A tail if/else.
pub fn sign(n: i32) -> i32 {
    if n < 0 { -1 } else if n == 0 { 0 } else { 1 }
}

pub fn spawns() -> i32 {
    let handle = std::thread::spawn(|| 40 + 2);
    std::thread::sleep(std::time::Duration::from_millis(1));
    let value = handle.join().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    tx.send(value).unwrap();
    drop(tx);
    rx.recv().unwrap_or(0)
}

pub fn exits(code: i32) {
    if code != 0 {
        std::process::exit(code);
    }
}
