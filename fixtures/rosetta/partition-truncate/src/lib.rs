use std::cmp::Ordering;

const NEAR_CONTACT_LIMIT: usize = 3;

pub fn near_contacts(mut nearest: Vec<(f64, usize)>) -> Vec<(f64, usize)> {
    let by_distance = |a: &(f64, usize), b: &(f64, usize)| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1));
    if nearest.len() > NEAR_CONTACT_LIMIT {
        nearest.select_nth_unstable_by(NEAR_CONTACT_LIMIT - 1, by_distance);
        nearest.truncate(NEAR_CONTACT_LIMIT);
    }
    nearest.sort_by(by_distance);
    nearest
}

pub fn partition(values: &mut [u8], index: usize) -> (&mut [u8], &mut u8, &mut [u8]) {
    <[u8]>::select_nth_unstable_by(values, index, |a, b| b.cmp(a))
}

pub fn shorten(values: &mut Vec<u8>, length: usize) {
    Vec::truncate(values, length);
}

pub struct Local;
impl Local {
    fn select_nth_unstable_by(
        &mut self,
        _index: usize,
        _compare: impl FnMut(&u8, &u8) -> Ordering,
    ) {
    }
    fn truncate(&mut self, _length: usize) {}
}

pub fn homonyms(mut local: Local, mut text: String) {
    local.select_nth_unstable_by(3, u8::cmp);
    local.truncate(1);
    text.truncate(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_and_truncate_mutate_the_input() {
        assert_eq!(
            near_contacts(vec![(5.0, 0), (1.0, 1), (3.0, 2), (1.0, 3), (1.0, 4)]),
            vec![(1.0, 1), (1.0, 3), (1.0, 4)]
        );
        let mut descending = [1, 5, 3, 4, 2, 3];
        let (left, middle, right) = partition(&mut descending, 2);
        assert_eq!(*middle, 3);
        assert!(left.iter().all(|item| item >= middle));
        assert!(right.iter().all(|item| item <= middle));
        let mut values = Vec::with_capacity(20);
        values.extend([1, 2, 3]);
        let capacity = values.capacity();
        shorten(&mut values, 10);
        assert_eq!(values, [1, 2, 3]);
        shorten(&mut values, 3);
        assert_eq!(values, [1, 2, 3]);
        shorten(&mut values, 2);
        assert_eq!(values, [1, 2]);
        shorten(&mut values, 0);
        assert!(values.is_empty());
        assert_eq!(values.capacity(), capacity);
        assert!(
            std::panic::catch_unwind(|| {
                partition(&mut [1], 1);
            })
            .is_err()
        );
    }
}
