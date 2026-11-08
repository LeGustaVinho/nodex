/// A shape with an area.
pub trait Shape {
    fn area(&self) -> f64;
    fn name(&self) -> String {
        "shape".to_owned()
    }
}

pub struct Circle {
    pub radius: f64,
}

pub struct Square(pub f64);

impl Shape for Circle {
    fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }

    fn name(&self) -> String {
        "circle".to_owned()
    }
}

impl Shape for Square {
    fn area(&self) -> f64 {
        self.0 * self.0
    }
}

/// The area of every shape added up.
pub fn total_area(shapes: &[Box<dyn Shape>]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}
