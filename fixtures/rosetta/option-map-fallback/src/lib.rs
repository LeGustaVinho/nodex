pub fn group_name(group: Option<&String>) -> String {
    group.map_or(String::new(), |name| name.clone())
}

pub fn eager(value: Option<String>, default: Option<usize>) -> usize {
    value.map_or(default.unwrap_or_default(), |text| {
        let size = Some(text.len());
        size.unwrap_or_default()
    })
}

pub fn lazy(value: Option<String>, default: Option<usize>) -> usize {
    Option::map_or_else(
        value,
        || {
            let stored = default;
            stored.unwrap_or_default()
        },
        |text| {
            let size = Some(text.len());
            size.unwrap_or_default()
        },
    )
}

pub fn result_control(value: Result<String, usize>) -> usize {
    value.map_or_else(|err| err, |value| value.len())
}

pub fn factories(value: Option<String>) -> usize {
    fn make_fallback() -> impl FnOnce() -> usize {
        || 0
    }
    fn make_mapper() -> impl FnOnce(String) -> usize {
        |text| text.len()
    }
    value.map_or_else(make_fallback(), make_mapper())
}

pub struct Local;
impl Local {
    pub fn map_or(self, fallback: u8, _: impl FnOnce(u8) -> u8) -> u8 {
        fallback
    }
}

pub fn shadowed(value: Local) -> u8 {
    value.map_or(0, |n| n + 1)
}
