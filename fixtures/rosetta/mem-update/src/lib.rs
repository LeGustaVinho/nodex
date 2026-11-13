use core::mem::{replace as install, take as reset};

pub struct LockState {
    pub locks: Vec<u32>,
    pub pending_locks: Vec<u32>,
}

pub fn reconcile_locks(state: &mut LockState) -> (Vec<u32>, Vec<u32>) {
    (
        std::mem::take(&mut state.locks),
        core::mem::take(&mut state.pending_locks),
    )
}

pub struct Encounter {
    pub hit_layers: Option<Vec<u8>>,
}

pub fn resolve_hit(encounter: &mut Encounter, outer: Option<Vec<u8>>) -> Vec<u8> {
    std::mem::replace(&mut encounter.hit_layers, outer).unwrap_or_default()
}

pub fn resolve_hit_with_fallback(encounter: &mut Encounter, outer: Option<Vec<u8>>) -> Vec<u8> {
    std::mem::replace(&mut encounter.hit_layers, outer).unwrap_or_else(Vec::new)
}

pub fn forwarded(slot: &mut String, new_name: String) -> (String, String) {
    let old = install::<String>(slot, new_name);
    (old, reset(slot))
}

mod local {
    pub fn take(value: &mut String) -> String {
        value.clone()
    }

    pub fn replace(_: &mut String, other: String) -> String {
        other
    }
}

pub fn local_updates(slot: &mut String, new_name: String) -> (String, String) {
    (local::take(slot), local::replace(slot, new_name))
}

pub fn option_take(slot: &mut Option<String>) -> Option<String> {
    slot.take()
}
