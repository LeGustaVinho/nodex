mod state;

use state::Tick as Renamed;
use state::{Count, Float, Position, Tick, js_max, multiplier};

pub fn lock_tick(age: f64, minimum: f64) -> Tick {
    js_max(age, minimum) as Tick
}

pub fn renamed(value: Float) -> Renamed {
    value as Renamed
}

pub fn chained(value: f64) -> Count {
    value as state::Count
}

pub fn annotated(value: f64) -> Tick {
    let local: Float = value;
    local as Tick
}

pub fn via_call() -> Tick {
    multiplier() as Tick
}

pub fn via_field(point: Position) -> Tick {
    point.x as Tick
}

pub fn via_nested(value: f64) -> u16 {
    (value as Tick) as u16
}

pub fn shadowed_rhs(value: f64) -> u8 {
    value as state::shadow::Small
}

pub fn generic_target(value: f64) -> state::Generic<i16> {
    value as state::Generic<i16>
}

pub fn conditional_target(value: f64) -> state::Conditional {
    value as state::Conditional
}

#[cfg(not(feature = "narrow"))]
pub fn conditional_module_target(
    value: f64,
) -> (
    state::inner_conditional::Tick,
    state::outer_conditional::Tick,
) {
    (
        value as state::inner_conditional::Tick,
        value as state::outer_conditional::Tick,
    )
}

pub fn associated_target(value: f64) -> <u8 as state::Clock>::Time {
    value as <u8 as state::Clock>::Time
}

pub fn nominal_source(value: state::nominal::Tick) -> u8 {
    value as u8
}

pub fn newtype_field(value: state::nominal::Number) -> u8 {
    value.0 as u8
}

pub fn pointer_target(value: usize) -> state::Address {
    value as state::Address
}
