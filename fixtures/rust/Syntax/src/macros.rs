macro_rules! log_twice {
    ($msg:expr) => {
        println!("{}", $msg);
        println!("{}", $msg);
    };
}

macro_rules! square {
    ($x:expr) => {
        $x * $x
    };
}

/// Macro calls of the standard library and of the project.
pub fn macros(items: &[i32]) -> Vec<i32> {
    let n = square!(3);
    log_twice!("hi");
    let list = vec![1, 2, n];
    let text = format!("{} items", items.len());
    println!("{text}");
    eprintln!("warning: {}", list.len());
    debug_assert!(n > 0);
    let matches = matches!(n, 1..=9);
    let line = line!();
    dbg!(matches);
    list
}
