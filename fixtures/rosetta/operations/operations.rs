// The same logic as operations.cs / operations.ts, written the Rust way.
pub fn operations(a: i32, b: i32, name: Option<&str>, items: &mut Vec<usize>) -> String {
    let sum = a + b * 2;
    let big = a.max(b) >= 10 && !(a == b);
    let missing = name.is_none();
    let mut text = name.unwrap_or("").trim().to_uppercase();
    if text.starts_with("X") || text.contains("Y") {
        text = text.replace("X", "Z");
    }
    let parts: Vec<&str> = text.split(",").collect();
    let joined = parts.join(";");
    let count = items.len() + parts.len();
    let parsed = joined.parse::<i32>().unwrap_or(0);
    let root = ((a - b).abs() as f64).sqrt() % 3.0;
    let mut i = 0;
    i += 1;
    i += count;
    items.push(i);
    println!("Total: {}", sum);
    parsed.to_string()
}
