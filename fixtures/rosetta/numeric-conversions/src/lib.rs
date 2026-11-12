use std::num::{NonZero, TryFromIntError};

pub fn in_bucket(tick: u64, bucket: u16, bucket_size: u64) -> bool {
    tick / bucket_size == u64::from(bucket)
}

pub fn checked_count(records: &[u8]) -> u16 {
    u16::try_from(records.len()).expect("too many records")
}

pub fn narrow(value: i16) -> Result<u8, TryFromIntError> {
    <u8 as std::convert::TryFrom<i16>>::try_from(value)
}

pub fn to_platform_size(value: u64) -> Result<usize, TryFromIntError> {
    usize::try_from(value)
}

pub fn to_float(value: i16) -> f64 {
    f64::from(value)
}

pub fn other_conversions(value: u16, small: NonZero<u8>, signed: NonZero<i8>) {
    let _ = u64::try_from(value);
    let _ = u16::from(value);
    let _ = u8::from(true);
    let _ = NonZero::<u16>::from(small);
    let _ = NonZero::<u8>::try_from(signed);
}

pub enum State {
    Ready,
}

impl TryFrom<i8> for State {
    type Error = &'static str;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Ready),
            _ => Err("invalid state"),
        }
    }
}

impl From<State> for u8 {
    fn from(_: State) -> Self {
        1
    }
}

pub fn local_conversions(value: i8) -> Result<u8, &'static str> {
    let state = State::try_from(value)?;
    Ok(u8::from(state))
}

pub mod shadow {
    #[allow(non_camel_case_types)]
    pub struct u16(pub usize);

    impl u16 {
        pub fn try_from(value: usize) -> Option<Self> {
            (value % 2 == 0).then(|| Self(value))
        }
    }

    pub fn shadowed_conversion(value: usize) -> Option<u16> {
        u16::try_from(value)
    }
}
