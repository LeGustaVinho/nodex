pub fn normalize(slot: &mut Option<Vec<u32>>) -> Vec<u32> {
    let Some(queue) = slot.take_if(|queue| !queue.is_empty()) else {
        return Vec::new();
    };
    queue
}

pub fn keep_edited(slot: &mut Option<Vec<u32>>) -> Option<Vec<u32>> {
    Option::take_if(slot, |queue| {
        queue.push(10);
        queue.get(0).is_some_and(|value| *value > 100)
    })
}

pub struct Local;

impl Local {
    pub fn take_if(&mut self, test: impl FnOnce(&mut u32) -> bool) -> Option<u32> {
        let mut value = 1;
        test(&mut value).then_some(value)
    }
}

pub fn local_test(slot: &mut Local) -> Option<u32> {
    slot.take_if(|value| *value > 0)
}
