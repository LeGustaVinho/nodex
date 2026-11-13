use std::vec as repeat_vec;

pub fn integrate_heat(count: usize) -> Vec<Vec<f64>> {
    let mut deltas: Vec<Vec<f64>> = vec![Vec::new(); count];
    if let Some(room) = deltas.first_mut() {
        room.push(0.5);
    }
    deltas
}

struct Pulse { delivered: Vec<bool> }

fn advance_pulses(pulse: &mut Pulse, ships: &[u32]) {
    pulse.delivered = vec![false; ships.len()];
}

fn value<A, B>() -> bool { std::mem::size_of::<A>() < std::mem::size_of::<B>() }

pub fn with_generic_commas(count: usize) -> Vec<bool> {
    repeat_vec![value::<u32, u64>(); count]
}

pub fn zero_count(evaluations: &mut u32) -> Vec<u32> {
    std::vec![{ *evaluations += 1; 5 }; 0]
}

pub fn list_form() -> Vec<&'static str> {
    vec![";", { let label = "done"; label }]
}

mod local {
    macro_rules! vec {
        ($value:expr; $length:expr) => { $value };
    }

    pub fn local_vec() -> u32 {
        vec![7; 3]
    }
}
