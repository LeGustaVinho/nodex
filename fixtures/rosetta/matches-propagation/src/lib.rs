pub enum Value {
    String(String),
    Other,
}

pub struct Reader;

impl Reader {
    fn path(&self, module: Result<Value, &'static str>, _: &[&str]) -> Result<Value, &'static str> {
        module
    }

    pub fn specialized(&self, module: Result<Value, &'static str>, group: Option<&str>) -> Result<bool, &'static str> {
        let matched = matches!(self.path(module, &["itém", "group", "name"])?, Value::String(name) if Some(name.as_str()) == group);
        Ok(matched)
    }
}

fn id<A, B>(input: Result<A, B>) -> Result<A, B> {
    input
}

pub fn generic_and_nested(input: Result<Result<u8, ()>, ()>) -> Result<bool, ()> {
    Ok(std::matches!(id::<u8, ()>(input?)?, 10))
}

pub fn optional(input: Option<u8>) -> Option<bool> {
    Some(matches!((/* test this value */ input?), 10))
}

pub fn guard_only(flag: Result<bool, ()>) -> Result<bool, ()> {
    Ok(matches!(true, true if flag?))
}

pub fn nested_closure(input: Result<bool, ()>) -> Result<bool, ()> {
    Ok(matches!((|| -> Result<bool, ()> { Ok(input?) })(), Ok(true)))
}

pub fn literal_and_comment() -> bool {
    matches!("?" /* ? */, "?")
}

pub fn local_macro(input: Result<bool, ()>) -> Result<bool, ()> {
    macro_rules! matches {
        ($value:expr, $pattern:pat) => { true };
    }
    let _ = input;
    Ok(matches!(input?, true))
}

pub fn wrapped_macro(input: Result<bool, ()>) -> Result<bool, ()> {
    macro_rules! passthrough {
        ($value:expr) => { $value };
    }
    Ok(matches!(passthrough!(input?), true))
}
