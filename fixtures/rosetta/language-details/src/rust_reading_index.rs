use std::collections::HashMap;

pub struct Encounter {
    pub crew_index: HashMap<String, (usize, usize)>,
}
