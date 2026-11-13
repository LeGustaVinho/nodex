pub struct Probe { pub hp: f64 }
pub struct Field { pub expires_tick: u64, pub probe: Option<Probe> }

pub fn expired(field: &Field, tick: u64) -> bool {
    field.expires_tick <= tick || field.probe.as_ref().is_some_and(|probe| probe.hp <= 0.0)
}

pub fn advance_pulses(fields: &[Field]) -> usize {
    let mut deliveries = 0;
    for field in fields {
        if field.probe.as_ref().is_none_or(|probe| probe.hp <= 0.0) {
            continue;
        }
        deliveries += 1;
    }
    deliveries
}

pub fn qualified(probe: Option<Probe>, test: impl FnOnce(Probe) -> bool) -> bool {
    Option::is_none_or(probe, test)
}

mod local {
    struct Option;

    impl Option {
        fn is_some_and(self, test: impl FnOnce(i32) -> bool) -> bool { test(2) }
        fn is_none_or(self, test: impl FnOnce(i32) -> bool) -> bool { !test(3) }
    }

    pub fn local_predicates() -> (bool, bool) {
        (Option.is_some_and(|x| x > 1), Option.is_none_or(|x| x > 1))
    }
}
