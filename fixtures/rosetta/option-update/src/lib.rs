pub fn stage_layers(slot: &mut Option<Vec<u8>>, layers: Vec<u8>) -> Option<Vec<u8>> {
    slot.replace(layers)
}

fn candidate() -> bool {
    false
}

pub fn initialize_crew(slot: &mut Option<bool>) -> &mut bool {
    Option::get_or_insert(slot, candidate())
}

pub fn restore_layers(slot: &mut Option<Vec<u8>>, outer: Option<Vec<u8>>) -> Vec<u8> {
    std::mem::replace(slot, outer).unwrap_or_default()
}

pub fn read_or_default<T: Default>(result: Result<T, String>) -> T {
    result.unwrap_or_default()
}

pub struct Local;

impl Local {
    pub fn replace(&mut self, value: u8) -> u8 {
        value
    }

    pub fn get_or_insert(&mut self, value: u8) -> u8 {
        value
    }

    pub fn unwrap_or_default(&self) -> u8 {
        0
    }
}

pub fn local_calls(local: &mut Local) -> (u8, u8, u8) {
    (
        local.replace(3),
        local.get_or_insert(4),
        local.unwrap_or_default(),
    )
}
