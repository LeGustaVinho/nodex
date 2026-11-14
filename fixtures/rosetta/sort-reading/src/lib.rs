use std::cmp::Ordering;

pub fn nearest(contacts: &mut Vec<(f64, usize)>) {
    let by_distance = |a: &(f64, usize), b: &(f64, usize)| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1));
    contacts.sort_by(by_distance);
}

pub fn micro_jumps(contacts: &mut [(i32, usize)]) {
    contacts.sort_by(|a, b| {
        let order = a.0.cmp(&b.0);
        order.then_with(|| a.1.cmp(&b.1))
    });
}

pub fn unstable(contacts: &mut [(i32, usize)], compare: impl FnMut(&(i32, usize), &(i32, usize)) -> Ordering) {
    <[(i32, usize)]>::sort_unstable_by(contacts, compare);
}

pub fn entries(entries: &mut Vec<(&str, u32)>) {
    entries.sort_by_key(|(key, _)| key.parse::<u32>().ok().map_or((1, 0), |index| (0, index)));
}

pub struct Local;

impl Local {
    fn sort_by(&mut self, _compare: impl FnMut(&i32, &i32) -> Ordering) {}
}

pub fn homonym() {
    Local.sort_by(|a, b| a.cmp(b));
}

#[test]
fn stable_sort_keeps_the_order_of_ties() {
    let mut contacts = [(2, 0), (1, 1), (2, 2), (1, 3)];
    contacts.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(contacts, [(1, 1), (1, 3), (2, 0), (2, 2)]);
    unstable(&mut contacts, |a, b| b.0.cmp(&a.0));
    assert!(contacts.windows(2).all(|pair| pair[0].0 >= pair[1].0));
    let mut keys = vec![("z", 0), ("10", 1), ("1", 2), ("a", 3)];
    entries(&mut keys);
    assert_eq!(keys, [("1", 2), ("10", 1), ("z", 0), ("a", 3)]);
}
