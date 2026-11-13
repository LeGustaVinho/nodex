use std::sync::OnceLock as Cache;

pub struct Effects {
    index: Cache<Vec<u8>>,
    sources: Cache<Vec<u8>>,
}

impl Effects {
    pub fn moved(&mut self) {
        self.index.take();
        (self.sources.take());
    }

    pub fn remove_index(&mut self) -> Option<Vec<u8>> {
        Cache::take(&mut self.index)
    }

    pub fn index(&self, seed: Option<u8>) -> &Vec<u8> {
        self.index.get_or_init(|| {
            let first = seed.unwrap_or_default();
            vec![first]
        })
    }

    pub fn edit_if_ready(&mut self) {
        if let Some(index) = self.index.get_mut() {
            index.push(7);
        }
    }
}

pub struct Local(Vec<u8>);

impl Local {
    pub fn take(&mut self) -> Option<Vec<u8>> {
        None
    }

    pub fn get_mut(&mut self) -> Option<&mut Vec<u8>> {
        Some(&mut self.0)
    }

    pub fn get_or_init(&self, _: impl FnOnce() -> Vec<u8>) -> &Vec<u8> {
        &self.0
    }
}

pub fn local_calls(local: &mut Local) {
    local.take();
    local.get_mut();
    local.get_or_init(Vec::new);
}
