/// Closures and iterator pipelines.
pub fn closures(numbers: Vec<i32>) -> i32 {
    let offset = 10;
    let add = |x: i32| x + offset;
    let mut counter = 0;
    let mut bump = || counter += 1;
    bump();
    let owned = numbers.clone();
    let take = move || owned.len();

    let even_squares: i32 = numbers.iter().filter(|n| *n % 2 == 0).map(|n| n * n).sum();
    let names: Vec<String> = numbers.iter().map(|n| n.to_string()).collect();
    let first_big = numbers.iter().position(|n| *n > 100);
    let any = numbers.iter().any(|n| *n < 0);

    numbers.iter().for_each(|n| {
        let doubled = n * 2;
        println!("{doubled}");
    });

    let mut sorted = numbers.clone();
    sorted.sort_by(|a, b| {
        let key_a = a.abs();
        let key_b = b.abs();
        key_a.cmp(&key_b)
    });

    apply(add, 1) + even_squares + take() as i32 + counter
}

fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 {
    f(x)
}

pub fn returns_closure(k: i32) -> impl Fn(i32) -> i32 {
    move |x| x * k
}

pub fn boxed() -> Box<dyn Fn() -> String> {
    Box::new(|| String::from("boxed"))
}
