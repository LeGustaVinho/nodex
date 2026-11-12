use std::collections::HashMap;

pub fn reading_round(value: f64) -> f64 {
    value.round()
}

pub struct Encounter {
    pub crew_index: HashMap<String, (usize, usize)>,
}

pub struct ReadingCrossFile;
