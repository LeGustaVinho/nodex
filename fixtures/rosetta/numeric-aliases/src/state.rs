pub type Tick = i64;
pub type Count = Tick;
pub type Float = ::std::primitive::f64;
pub type Generic<T> = T;
pub type Address = *const i32;

pub fn js_max(a: f64, b: f64) -> f64 {
    a.max(b)
}

pub fn multiplier() -> Float {
    2.5
}

pub struct Position {
    pub x: Float,
}

pub mod nominal {
    pub enum Tick {
        Idle,
    }

    pub struct Number(pub i64);
}

pub mod shadow {
    #![allow(non_camel_case_types)]
    type i64 = u8;
    pub type Small = i64;
}

#[cfg(feature = "narrow")]
pub type Conditional = u8;
#[cfg(not(feature = "narrow"))]
pub type Conditional = i16;

pub mod inner_conditional {
    #![cfg(not(feature = "narrow"))]
    pub type Tick = i64;
}

#[cfg(not(feature = "narrow"))]
pub mod outer_conditional {
    pub type Tick = i64;
}

pub trait Clock {
    type Time;
}

impl Clock for u8 {
    type Time = i64;
}
