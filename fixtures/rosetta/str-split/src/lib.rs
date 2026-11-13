pub fn ticket(token: &str) -> Option<(&str, &str)> {
    let mut parts = token.split('.');
    let (Some(payload), Some(mac), None) = (parts.next(), parts.next(), parts.next()) else {
        return None;
    };
    Some((payload, mac))
}

pub fn decimal(text: &String) -> (&str, &str) {
    let (whole, fraction) = text.split_once('.').unwrap_or((text.as_str(), ""));
    (whole, fraction)
}

pub fn owner(value: &str) -> Result<(&str, &str), &'static str> {
    let (ship, owner) = value.split_once('=').ok_or("expected ship=owner")?;
    Ok((ship, owner))
}

pub fn collect_pieces(text: &str) -> Vec<String> {
    text.split(',').map(str::to_string).collect()
}

pub fn qualified(text: &str) -> Option<(&str, &str)> {
    str::split_once(text, |ch| ch == '-' || ch == '_')
}

pub fn split_slice(values: &[u32]) -> usize {
    let parts = values.split(|item| *item == 0);
    parts.count()
}

mod local {
    struct Text;

    impl Text {
        fn split(&self, _: char) -> u32 { 1 }
        fn split_once(&self, _: char) -> u32 { 2 }
    }

    pub fn local_split() -> (u32, u32) {
        (Text.split('.'), Text.split_once('.'))
    }
}
