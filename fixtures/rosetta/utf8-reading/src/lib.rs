use std::borrow::Cow;
use std::str::{Utf8Error, from_utf8 as decode_borrowed};
use std::string::{FromUtf8Error, String as OwnedText};

pub fn borrowed(bytes: &[u8]) -> Result<&str, Utf8Error> {
    decode_borrowed(bytes)
}

pub fn borrowed_primitive(bytes: &[u8]) -> Result<&str, Utf8Error> {
    str::from_utf8(bytes)
}

pub fn borrowed_match(bytes: &[u8]) -> bool {
    match std::str::from_utf8(bytes) {
        Ok(text) => !text.is_empty(),
        Err(_) => false,
    }
}

pub fn owned(bytes: Vec<u8>) -> Result<String, FromUtf8Error> {
    OwnedText::from_utf8(bytes)
}

pub fn recover(bytes: Vec<u8>) -> Vec<u8> {
    match String::from_utf8(bytes) {
        Ok(text) => text.into_bytes(),
        Err(error) => error.into_bytes(),
    }
}

pub fn lossy(bytes: &[u8]) -> Cow<'_, str> {
    String::from_utf8_lossy(bytes)
}

pub fn decode_number(bytes: &[u8]) -> Result<u32, std::num::ParseIntError> {
    let value = String::from_utf8_lossy(bytes).parse::<u32>()?;
    Ok(value)
}

pub fn snapshot(bytes: Result<&[u8], &'static str>) -> Result<String, &'static str> {
    let output = String::from_utf8(bytes?.to_vec()).map_err(|_| "bad UTF-8")?;
    Ok(output)
}

pub mod local {
    pub fn from_utf8(_: &[u8]) -> &'static str {
        "local"
    }

    pub struct String;

    impl String {
        pub fn from_utf8_lossy(_: &[u8]) -> &'static str {
            "local"
        }
    }
}

pub fn local_decoder(bytes: &[u8]) -> (&str, &str) {
    (
        local::from_utf8(bytes),
        local::String::from_utf8_lossy(bytes),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_ownership_and_invalid_bytes() {
        let valid = b"hello";
        assert_eq!(borrowed(valid).unwrap().as_ptr(), valid.as_ptr());
        assert!(matches!(lossy(valid), Cow::Borrowed(text) if text.as_ptr() == valid.as_ptr()));
        let bytes = b"hello".to_vec();
        let buffer = bytes.as_ptr();
        let converted = owned(bytes).unwrap();
        assert_eq!(converted.as_ptr(), buffer);
        assert_eq!(borrowed(b"a\xFF").unwrap_err().valid_up_to(), 1);
        assert_eq!(recover(b"a\xFF".to_vec()), b"a\xFF");
        assert!(matches!(lossy(b"a\xFF"), Cow::Owned(text) if text == "a\u{FFFD}"));
        assert!(decode_number(b"1\xFF").is_err());
    }
}
