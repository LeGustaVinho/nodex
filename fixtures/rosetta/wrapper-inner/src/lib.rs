use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

type Fresh = Mutex<HashMap<u8, Arc<OnceLock<Result<String, String>>>>>;

pub fn keep(fresh: Fresh) -> Vec<String> {
    let mut profiles = Vec::new();
    for (_, cell) in fresh.into_inner().expect("fresh profiles lock") {
        if let Some(Ok(profile)) = Arc::try_unwrap(cell)
            .ok()
            .and_then(|cell| cell.into_inner())
        {
            profiles.push(profile);
        }
    }
    profiles
}

pub fn skill_levels(values: OnceLock<Vec<u8>>) -> Vec<u8> {
    if let Some(levels) = values.into_inner() {
        levels
    } else {
        Vec::new()
    }
}

pub fn mutex_ufcs(mutex: Mutex<String>) -> Result<String, PoisonError<String>> {
    Mutex::into_inner(mutex)
}

pub fn once_ufcs(cell: OnceLock<String>) -> Option<String> {
    OnceLock::into_inner(cell)
}

pub fn arc_only(value: Arc<String>) -> Result<String, Arc<String>> {
    Arc::try_unwrap(value)
}

pub mod custom {
    pub struct Arc;
    pub struct OnceLock;
    pub struct Mutex;

    impl Arc {
        pub fn try_unwrap(self) -> u8 {
            1
        }
    }

    impl OnceLock {
        pub fn into_inner(self) -> u8 {
            2
        }
    }

    impl Mutex {
        pub fn into_inner(self) -> u8 {
            3
        }
    }
}

pub fn homonyms() -> (u8, u8, u8) {
    (
        custom::Arc.try_unwrap(),
        custom::OnceLock.into_inner(),
        custom::Mutex.into_inner(),
    )
}

pub fn other_std(
    arc: std::rc::Rc<u8>,
    cell: std::cell::OnceCell<u8>,
) -> (Result<u8, std::rc::Rc<u8>>, Option<u8>) {
    (std::rc::Rc::try_unwrap(arc), cell.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumes_wrappers_and_recovers_values() {
        let pointer = Arc::new(String::from("payload"));
        let weak = Arc::downgrade(&pointer);
        let other = Arc::clone(&pointer);
        let recovered = arc_only(pointer).unwrap_err();
        assert!(Arc::ptr_eq(&recovered, &other));
        drop(other);
        assert_eq!(arc_only(recovered).unwrap(), "payload");
        assert!(weak.upgrade().is_none());

        let mutex = Mutex::new(String::from("before"));
        let _ = std::panic::catch_unwind(|| {
            let mut guard = mutex.lock().unwrap();
            *guard = String::from("after");
            panic!("poison the mutex");
        });
        let poisoned = mutex_ufcs(mutex).unwrap_err();
        assert_eq!(poisoned.into_inner(), "after");

        assert_eq!(once_ufcs(OnceLock::new()), None);
        assert_eq!(
            once_ufcs(OnceLock::from(String::from("stored"))),
            Some(String::from("stored"))
        );
    }
}
