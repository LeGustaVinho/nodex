use indexmap::IndexMap as Ordered;

pub struct Locks {
    pub locks: Vec<u32>,
    pub pending_locks: Ordered<u32, i64>,
}

pub fn reconcile_locks(state: &mut Locks) -> (Vec<u32>, Ordered<u32, i64>) {
    (
        std::mem::take(&mut state.locks),
        std::mem::take(&mut state.pending_locks),
    )
}

pub struct Encounter {
    pub hit_layers: Option<Vec<u8>>,
}

pub fn resolve_hit(state: &mut Encounter, previous: Option<Vec<u8>>) -> Vec<u8> {
    std::mem::replace(&mut state.hit_layers, previous).unwrap_or_default()
}

pub fn from_result(input: Result<Vec<u8>, String>) -> Vec<u8> {
    input.unwrap_or_default()
}

pub fn from_return() -> Ordered<u32, i64> {
    fn candidate() -> Option<Ordered<u32, i64>> {
        None
    }
    candidate().unwrap_or_default()
}

mod local {
    pub struct Vec<T>(pub T);
    pub struct IndexMap<K, V>(pub K, pub V);

    impl Default for Vec<u8> {
        fn default() -> Self {
            Self(1)
        }
    }

    impl Default for IndexMap<u8, u8> {
        fn default() -> Self {
            Self(1, 2)
        }
    }
}

pub fn homonymous_vec(input: &mut local::Vec<u8>) -> local::Vec<u8> {
    std::mem::take(input)
}

pub fn homonymous_map(input: Option<local::IndexMap<u8, u8>>) -> local::IndexMap<u8, u8> {
    input.unwrap_or_default()
}

pub fn generic<T: Default>(input: Option<T>) -> T {
    input.unwrap_or_default()
}

pub fn reference_inside<'a>(input: &mut Option<&'a mut Vec<u8>>) -> Option<&'a mut Vec<u8>> {
    std::mem::take(input)
}
