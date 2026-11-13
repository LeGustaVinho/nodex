use std::collections::HashMap;

pub enum FittingError { UnknownType(u32) }

pub fn fit(catalog: &HashMap<u32, String>, type_id: u32) -> Result<String, FittingError> {
    let item = catalog.get(&type_id).ok_or(FittingError::UnknownType(type_id))?.clone();
    Ok(item)
}

pub fn ticket(claim: Result<Option<String>, &'static str>, mut error: impl FnMut() -> &'static str) -> Result<String, &'static str> {
    let owner_id = claim?.ok_or(error())?;
    Ok(owner_id)
}

pub fn qualified(option: Option<u8>, error: impl FnOnce() -> u32) -> Result<u8, u32> {
    Option::ok_or(option, error())
}

pub fn lazy(option: Option<u8>, error: impl FnOnce() -> u32) -> Result<u8, u32> {
    option.ok_or_else(error)
}

mod local {
    struct Option;

    impl Option {
        fn ok_or(self, error: u32) -> u32 { error + 1 }
    }

    pub fn local_ok_or() -> u32 { Option.ok_or(42) }
}
