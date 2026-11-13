pub fn module_practice(local_shield: bool, rank: impl FnOnce(&str) -> Option<u8>) -> Option<u8> {
    let compensation = local_shield.then(|| rank("shield-compensation")).flatten();
    compensation
}

pub fn multiple_layers(value: Option<Option<Option<u8>>>) -> Option<Option<u8>> {
    Option::flatten(value)
}

pub struct Entry { pub module: Option<usize>, pub source: Option<usize> }

pub fn view(entry: &Entry, include_module: bool) -> Option<(usize, usize)> {
    let module = entry.module.zip(entry.source).filter(|_| include_module);
    module
}

pub fn validation(charge_volume: Option<f64>, module_capacity: Option<f64>) -> bool {
    charge_volume.zip(module_capacity).is_some_and(|(volume, cap)| volume > cap)
}

pub fn flatten_items(nested: Vec<Vec<u32>>) -> Vec<u32> {
    nested.into_iter().flatten().collect()
}

pub fn zip_items(left: &[u32], right: &[u32]) -> usize {
    left.iter().zip(right.iter()).count()
}

mod local {
    struct Option;

    impl Option {
        fn flatten(self) -> u32 { 1 }
        fn zip(self, _: Self) -> u32 { 2 }
    }

    pub fn local_composition() -> (u32, u32) {
        (Option.flatten(), Option.zip(Option))
    }
}
