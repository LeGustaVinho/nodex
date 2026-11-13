pub fn launcher_type_id(payload: Option<String>, type_id: Option<u32>) -> Option<u32> {
    payload.as_ref().and(type_id)
}

pub fn choose(
    primary: impl FnOnce() -> Result<Option<String>, &'static str>,
    candidate: impl FnOnce() -> Result<Option<u32>, &'static str>,
) -> Result<Option<u32>, &'static str> {
    let selected = primary()?.and(candidate()?);
    Ok(selected)
}

pub fn qualified(first: Option<String>, second: Option<u32>) -> Option<u32> {
    Option::and::<u32>(first, second)
}

pub struct Local;

impl Local {
    pub fn and(self, candidate: Option<u32>) -> Option<u32> {
        candidate
    }
}

pub fn local_and(first: Local, second: Option<u32>) -> Option<u32> {
    first.and(second)
}
