//! Binary trace protocol (see the Unity package's shared runtime `T.cs`).
//!
//! After a handshake line `NODEX1 <token>\n`, the program sends frames: `u32 length` followed by
//! records. Each record (little-endian): `u8 kind, i32 thread, i64 micros, i32 method, i32 arg,
//! u16 text length, UTF-8 text`.

/// Fixed part of a record, in bytes.
const HEADER: usize = 1 + 4 + 8 + 4 + 4 + 2;
/// Frames larger than this are rejected as corrupt.
const MAX_FRAME: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordKind {
    Enter,
    Step,
    Exit,
    Local,
    Return,
    Arg,
    /// An exception was thrown on the thread (`method` is 0: it belongs to the current statement).
    Throw,
    Dropped,
    /// First record of a frame: `thread` is the process id, `arg` the frame number. Lets Nodex
    /// skip frames it already received when it reads a crash tail file.
    Sequence,
    /// Type of a local/argument in the indicated slot. The text is a portable
    /// descriptor, newline, and native spelling. A blank descriptor is Rust native text.
    LocalType,
    ArgType,
    Unknown(u8),
}

impl From<u8> for RecordKind {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::Enter,
            2 => Self::Step,
            3 => Self::Exit,
            4 => Self::Local,
            5 => Self::Return,
            6 => Self::Arg,
            7 => Self::Throw,
            9 => Self::Dropped,
            10 => Self::Sequence,
            11 => Self::LocalType,
            12 => Self::ArgType,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub kind: RecordKind,
    pub thread: i32,
    /// Microseconds since the traced process started tracing.
    pub micros: i64,
    pub method: i32,
    /// Point index (`Step`), local slot (`Local`), parameter index (`Arg`), count (`Dropped`).
    pub arg: i32,
    pub text: Option<String>,
}

/// Incremental decoder: feed arbitrary chunks, get complete records back.
#[derive(Default)]
pub struct Decoder {
    buf: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CorruptStream(pub String);

impl Decoder {
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Record>, CorruptStream> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        let mut pos = 0;
        while self.buf.len() - pos >= 4 {
            let len = u32::from_le_bytes(self.buf[pos..pos + 4].try_into().expect("4 bytes")) as usize;
            if len > MAX_FRAME {
                return Err(CorruptStream(format!("frame of {len} bytes")));
            }
            if self.buf.len() - pos - 4 < len {
                break;
            }
            decode_frame(&self.buf[pos + 4..pos + 4 + len], &mut out)?;
            pos += 4 + len;
        }
        self.buf.drain(..pos);
        Ok(out)
    }
}

fn decode_frame(mut data: &[u8], out: &mut Vec<Record>) -> Result<(), CorruptStream> {
    while !data.is_empty() {
        if data.len() < HEADER {
            return Err(CorruptStream("truncated record".into()));
        }
        let kind = RecordKind::from(data[0]);
        let thread = i32::from_le_bytes(data[1..5].try_into().expect("4"));
        let micros = i64::from_le_bytes(data[5..13].try_into().expect("8"));
        let method = i32::from_le_bytes(data[13..17].try_into().expect("4"));
        let arg = i32::from_le_bytes(data[17..21].try_into().expect("4"));
        let text_len = u16::from_le_bytes(data[21..23].try_into().expect("2")) as usize;
        if data.len() < HEADER + text_len {
            return Err(CorruptStream("truncated text".into()));
        }
        let text = (text_len > 0).then(|| String::from_utf8_lossy(&data[HEADER..HEADER + text_len]).into_owned());
        out.push(Record { kind, thread, micros, method, arg, text });
        data = &data[HEADER + text_len..];
    }
    Ok(())
}

/// Encodes records the way the runtime does (used by tests and tools).
pub fn encode_frame(records: &[Record]) -> Vec<u8> {
    let mut payload = Vec::new();
    for r in records {
        let kind = match r.kind {
            RecordKind::Enter => 1,
            RecordKind::Step => 2,
            RecordKind::Exit => 3,
            RecordKind::Local => 4,
            RecordKind::Return => 5,
            RecordKind::Arg => 6,
            RecordKind::Throw => 7,
            RecordKind::Dropped => 9,
            RecordKind::Sequence => 10,
            RecordKind::LocalType => 11,
            RecordKind::ArgType => 12,
            RecordKind::Unknown(k) => k,
        };
        payload.push(kind);
        payload.extend_from_slice(&r.thread.to_le_bytes());
        payload.extend_from_slice(&r.micros.to_le_bytes());
        payload.extend_from_slice(&r.method.to_le_bytes());
        payload.extend_from_slice(&r.arg.to_le_bytes());
        let text = r.text.as_deref().unwrap_or("").as_bytes();
        payload.extend_from_slice(&(text.len() as u16).to_le_bytes());
        payload.extend_from_slice(text);
    }
    let mut frame = (payload.len() as u32).to_le_bytes().to_vec();
    frame.extend(payload);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(kind: RecordKind, arg: i32, text: Option<&str>) -> Record {
        Record { kind, thread: 1, micros: 42, method: 7, arg, text: text.map(str::to_owned) }
    }

    #[test]
    fn decodes_frames_split_across_chunks() {
        let records = vec![rec(RecordKind::Enter, 0, None), rec(RecordKind::Local, 2, Some("total = 1é"))];
        let mut bytes = encode_frame(&records);
        bytes.extend(encode_frame(&[rec(RecordKind::Exit, 0, None)]));
        let mut d = Decoder::default();
        let mut got = Vec::new();
        for chunk in bytes.chunks(5) {
            got.extend(d.feed(chunk).unwrap());
        }
        assert_eq!(got.len(), 3);
        assert_eq!(got[1], records[1]);
    }

    #[test]
    fn rejects_garbage() {
        let mut d = Decoder::default();
        assert!(d.feed(&[3, 0, 0, 0, 1, 2, 3]).is_err());
    }
}
