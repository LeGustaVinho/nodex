use crate::expressions::Point;

pub enum Shape {
    Circle { radius: f64 },
    Rect(f64, f64),
    Empty,
}

/// Patterns in let, match and function parameters.
pub fn patterns(shape: &Shape, values: &[i32], Point { x, y }: Point) -> f64 {
    let area = match shape {
        Shape::Circle { radius } if *radius > 0.0 => 3.14 * radius * radius,
        Shape::Circle { .. } => 0.0,
        Shape::Rect(w, h) => w * h,
        Shape::Empty => 0.0,
    };
    let [first, .., last] = values else {
        return area;
    };
    let total @ 1..=9 = *first else {
        return 0.0;
    };
    match values {
        [] => {}
        [one] => println!("{one}"),
        [head, tail @ ..] => println!("{head} {}", tail.len()),
    }
    let (ref a, mut b) = (x, y);
    b += *a;
    area + b + *last as f64 + total as f64
}
