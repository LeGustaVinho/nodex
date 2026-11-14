use std::collections::HashMap;

pub struct Blueprint {
    pub charge_id: Option<u32>,
}

pub struct Module {
    pub loaded_charge_id: std::option::Option<u32>,
}

pub fn ammunition_ids(
    ids: &mut Vec<u32>,
    blueprints: &[Blueprint],
    modules: &[Module],
    index: usize,
) {
    ids.extend(blueprints[index].charge_id);
    ids.extend(modules[index].loaded_charge_id);
}

pub fn settle(values: &mut Vec<String>, ships: &[u32], mut observed: impl FnMut(u32)) {
    values.extend(ships.iter().map(|&ship| {
        observed(ship);
        format!("ship {ship}")
    }));
}

pub fn borrowed_items(values: &mut Vec<u32>, source: &[u32]) {
    Extend::extend(values, source);
}

pub fn borrowed_option(values: &mut Vec<u32>, source: &Option<u32>) {
    values.extend(source);
}

pub fn reference_items<'a>(values: &mut Vec<&'a u32>, source: &'a [u32]) {
    values.extend(source);
}

pub fn local_binding(values: &mut Vec<u32>, source: Option<u32>) {
    values.extend(source);
}

fn mobile_event(moving: bool) -> Option<String> {
    moving.then(|| String::from("moving"))
}

pub fn emit(events: &mut Vec<String>, moving: bool) {
    events.extend(mobile_event(moving));
}

pub fn constructor(values: &mut Vec<u32>, value: u32) {
    values.extend(Some(value));
}

pub struct Local;
impl Local {
    fn extend(&mut self, _values: impl IntoIterator<Item = u32>) {}
}

pub fn homonyms(mut local: Local, mut text: String, mut map: HashMap<u32, u32>) {
    local.extend([1, 2]);
    text.extend(['a', 'b']);
    map.extend([(1, 2)]);
}

pub mod shadowed {
    pub struct Option;

    impl IntoIterator for Option {
        type Item = u32;
        type IntoIter = std::array::IntoIter<u32, 2>;

        fn into_iter(self) -> Self::IntoIter {
            [4, 5].into_iter()
        }
    }

    pub struct Module {
        pub loaded_charge_id: Option,
    }

    pub fn multiple(values: &mut Vec<u32>, source: Option, module: Module) {
        values.extend(source);
        values.extend(module.loaded_charge_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drives_sources_and_keeps_the_existing_prefix() {
        let mut events = Vec::new();
        let mut values = vec![String::from("old")];
        settle(&mut values, &[9, 4], |ship| events.push(ship));
        assert_eq!(values, ["old", "ship 9", "ship 4"]);
        assert_eq!(events, [9, 4]);

        let mut ids = vec![12];
        let blueprints = [Blueprint { charge_id: None }];
        let modules = [Module {
            loaded_charge_id: Some(13),
        }];
        ammunition_ids(&mut ids, &blueprints, &modules, 0);
        borrowed_items(&mut ids, &[14, 15]);
        borrowed_option(&mut ids, &Some(16));
        assert_eq!(ids, [12, 13, 14, 15, 16]);
        shadowed::multiple(
            &mut ids,
            shadowed::Option,
            shadowed::Module {
                loaded_charge_id: shadowed::Option,
            },
        );
        assert_eq!(&ids[5..], [4, 5, 4, 5]);
    }
}
