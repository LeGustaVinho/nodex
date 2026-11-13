use std::collections::VecDeque;

pub fn write_batch(batch: &mut Vec<String>, mut write: impl FnMut(String)) {
    for entry in batch.drain(..) {
        write(entry);
    }
}

pub fn trim_visual(retained: &mut Vec<String>, excess: usize) {
    retained.drain(..excess);
}

pub fn early_return(batch: &mut Vec<String>) -> Option<String> {
    for entry in batch.drain(..) {
        return Some(entry);
    }
    None
}

pub fn early_break(batch: &mut Vec<String>) {
    for _entry in batch.drain(..) {
        break;
    }
}

pub fn saved(batch: &mut Vec<String>) -> std::vec::Drain<'_, String> {
    Vec::drain(batch, ..)
}

pub fn deque(batch: &mut VecDeque<String>) {
    batch.drain(..);
}

mod local {
    struct Vec;

    impl Vec {
        fn drain(&mut self, _: std::ops::RangeFull) {}
    }

    pub fn local_drain() {
        let mut batch = Vec;
        batch.drain(..);
    }
}
