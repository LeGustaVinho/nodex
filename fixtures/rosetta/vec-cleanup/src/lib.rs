use std::collections::VecDeque;

pub fn module_profile(
    profiles: Result<Vec<String>, &'static str>,
    index: usize,
) -> Result<String, &'static str> {
    let profile = profiles?.swap_remove(index);
    Ok(profile)
}

pub fn move_crew(source: &mut Vec<String>, destination: &mut Vec<String>, index: usize) {
    let crew = source.remove(index);
    destination.push(crew);
}

pub fn take(values: &mut Vec<String>, index: usize) -> String {
    Vec::swap_remove(values, index)
}

pub fn remove_ufcs(values: &mut Vec<String>, index: usize) -> String {
    Vec::remove(values, index)
}

pub fn sorted_unique(mut ids: Vec<u8>) -> Vec<u8> {
    ids.sort_unstable();
    ids.dedup();
    ids
}

pub fn dedup_ufcs(values: &mut Vec<u8>) {
    Vec::dedup(values);
}

pub struct Local;
impl Local {
    fn swap_remove(&mut self, _index: usize) {}
    fn remove(&mut self, _index: usize) {}
    fn dedup(&mut self) {}
}

pub fn homonyms(mut local: Local, mut queue: VecDeque<u8>, mut text: String) {
    local.swap_remove(3);
    local.remove(1);
    local.dedup();
    queue.remove(1);
    text.remove(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn removal_order_and_adjacent_dedup_are_distinct() {
        let mut values = strings(&["a", "b", "c", "d"]);
        assert_eq!(take(&mut values, 1), "b");
        assert_eq!(values, ["a", "d", "c"]);
        assert_eq!(take(&mut values, 2), "c");
        assert_eq!(values, ["a", "d"]);

        let mut source = strings(&["a", "b", "c", "d"]);
        let mut destination = Vec::new();
        move_crew(&mut source, &mut destination, 1);
        assert_eq!(source, ["a", "c", "d"]);
        assert_eq!(destination, ["b"]);
        assert!(std::panic::catch_unwind(|| take(&mut Vec::new(), 0)).is_err());
        assert!(std::panic::catch_unwind(|| remove_ufcs(&mut Vec::new(), 0)).is_err());

        #[derive(Debug)]
        struct Entry(u8, &'static str);
        impl PartialEq for Entry {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        let mut entries = vec![
            Entry(1, "first"),
            Entry(1, "adjacent"),
            Entry(2, "different"),
            Entry(1, "later"),
        ];
        entries.dedup();
        assert_eq!(
            entries.iter().map(|item| item.1).collect::<Vec<_>>(),
            ["first", "different", "later"]
        );
        assert_eq!(sorted_unique(vec![2, 1, 2, 3, 2]), [1, 2, 3]);
    }
}
