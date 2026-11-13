pub fn choose(
    primary: impl FnOnce() -> Result<Option<u32>, &'static str>,
    fallback: impl FnOnce() -> Result<Option<u32>, &'static str>,
) -> Result<Option<u32>, &'static str> {
    let selected = primary()?.or(fallback()?);
    Ok(selected)
}

pub fn qualified(first: Option<u32>, second: Option<u32>) -> Option<u32> {
    Option::or(first, second)
}

pub struct Local;

impl Local {
    pub fn or(self, alternative: Option<u32>) -> Option<u32> {
        alternative
    }
}

pub fn local_or(first: Local, second: Option<u32>) -> Option<u32> {
    first.or(second)
}
