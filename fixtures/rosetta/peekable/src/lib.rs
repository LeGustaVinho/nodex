use std::{collections::BinaryHeap, iter::Peekable};

pub struct Stored { pub tick: u64 }

pub fn replay(mut inputs: Peekable<impl Iterator<Item = Stored>>, tick: u64) -> Vec<Stored> {
    let mut queued = Vec::new();
    while let Some(stored) = inputs.next_if(|stored| stored.tick <= tick) {
        queued.push(stored);
    }
    queued
}

pub struct Effect { pub source: u32, pub jam: bool }

pub fn can_target(effects: &[Effect], target: u32) -> bool {
    let mut jams = effects.iter().filter(|effect| effect.jam).peekable();
    jams.peek().is_none() || jams.any(|effect| effect.source == target)
}

pub fn qualified<'a>(values: &'a mut Peekable<impl Iterator<Item = u32>>) -> Option<&'a u32> {
    Peekable::peek(values)
}

pub fn heap_peek(heap: &BinaryHeap<u32>) -> Option<&u32> { heap.peek() }

mod local {
    struct Peekable;

    impl Peekable {
        fn peek(&self) -> Option<u32> { None }
        fn next_if(&mut self, _: impl FnOnce(&u32) -> bool) -> Option<u32> { None }
    }

    pub fn local_peekable() -> (Option<u32>, Option<u32>) {
        let mut iter = Peekable;
        (iter.peek(), iter.next_if(|&item| item > 0))
    }
}
