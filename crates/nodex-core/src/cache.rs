//! On-disk cache of lowered files, so reopening a project lowers only the files that changed.
//!
//! One bundle per project root, at `<base>/<hash of the root>/files.bin`. Each entry is a file's
//! [`FileIr`] keyed by its project-relative path, size and modification time; the whole bundle
//! is keyed by a [`stamp`] of everything else a lowering depends on (Nodex version, embedded
//! catalog, bundle layout, the running build). Anything unexpected in the bundle (missing,
//! truncated, another layout, an entry that does not decode) only means those files are
//! lowered again: the cache never makes an open fail.
//!
//! Entries are MessagePack with named fields ([`nodex_ir::Symbol`] leaves default fields out of
//! its serialized form, which formats that rely on field order cannot read back), compressed
//! with LZ4: field names and the file path in every span repeat a lot (about 6× smaller).

use std::collections::HashMap;
use std::fs::Metadata;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nodex_ir::FileIr;

/// Start of every bundle; the last byte is the layout version (bump it when the layout changes).
const MAGIC: &[u8; 8] = b"NODEXIR\x01";
const BUNDLE: &str = "files.bin";
/// Files changed this recently are not cached: a second change within the file system's
/// timestamp resolution could keep the same size and modification time.
const RACY: Duration = Duration::from_secs(2);

/// What a cached entry must match on disk: size and modification time (nanoseconds since 1970).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stat {
    pub size: u64,
    pub mtime: u64,
}

impl Stat {
    pub fn of(meta: &Metadata) -> Option<Self> {
        let mtime = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some(Self { size: meta.len(), mtime: u64::try_from(mtime.as_nanos()).ok()? })
    }
}

/// Changes whenever a cached lowering could differ from a fresh one: the Nodex version, the
/// embedded catalog, the bundle layout and the running executable (a rebuild during
/// development changes the lowering without changing the version).
fn stamp() -> &'static [u8; 32] {
    static STAMP: OnceLock<[u8; 32]> = OnceLock::new();
    STAMP.get_or_init(|| {
        let mut h = blake3::Hasher::new();
        h.update(MAGIC);
        h.update(env!("CARGO_PKG_VERSION").as_bytes());
        for source in crate::catalog::embedded_sources() {
            h.update(&(source.len() as u64).to_le_bytes());
            h.update(source.as_bytes());
        }
        if let Some(stat) = std::env::current_exe().ok().and_then(|exe| std::fs::metadata(exe).ok()).and_then(|m| Stat::of(&m)) {
            h.update(&stat.size.to_le_bytes());
            h.update(&stat.mtime.to_le_bytes());
        }
        *h.finalize().as_bytes()
    })
}

/// The cache folder of the project at `root` (a canonical path) under `base`.
pub fn dir(base: &Path, root: &Path) -> PathBuf {
    let hash = blake3::hash(root.to_string_lossy().as_bytes()).to_hex();
    base.join(&hash[..16])
}

/// The bundle file of the project at `root` (a canonical path) under `base`.
pub fn bundle(base: &Path, root: &Path) -> PathBuf {
    dir(base, root).join(BUNDLE)
}

/// Removes the cache of the project at `root` under `base` (so its next open is a cold one).
pub fn clear(base: &Path, root: &Path) -> std::io::Result<()> {
    let root = crate::project::dunce_canonical(root)?;
    match std::fs::remove_dir_all(dir(base, &root)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Encodes a lowered file for the cache: MessagePack, LZ4-compressed after its length.
pub fn encode(ir: &FileIr) -> Option<Vec<u8>> {
    Some(lz4_flex::compress_prepend_size(&rmp_serde::to_vec_named(ir).ok()?))
}

/// Reads back what [`encode`] wrote, or `None` when the bytes are not such an encoding.
pub fn decode(bytes: &[u8]) -> Option<FileIr> {
    let (len, compressed) = bytes.split_first_chunk::<4>()?;
    // LZ4 expands at most about 255 times: a larger length is corrupt, and must not be allocated.
    let len = u32::from_le_bytes(*len) as usize;
    if len > compressed.len().saturating_mul(255) {
        return None;
    }
    let plain = lz4_flex::decompress(compressed, len).ok()?;
    rmp_serde::from_slice(&plain).ok()
}

/// A loaded bundle: lookups by path, decoding on demand (from several threads at once).
pub struct LowerCache {
    file: PathBuf,
    root: String,
    data: Vec<u8>,
    entries: HashMap<String, Slot>,
}

/// Where an entry lies in the bundle, with what it must match.
struct Slot {
    stat: Stat,
    checksum: [u8; 8],
    range: Range<usize>,
}

/// One file of a bundle being written: its stat and its encoded [`FileIr`].
pub struct Entry<'a> {
    pub rel: &'a str,
    pub stat: Stat,
    pub bytes: &'a [u8],
}

impl LowerCache {
    /// Reads the bundle of `root` under `base`. Empty when there is none or it cannot be used.
    pub fn load(base: &Path, root: &Path) -> Self {
        let file = bundle(base, root);
        let root = root.to_string_lossy().into_owned();
        let mut cache = Self { file, root, data: Vec::new(), entries: HashMap::new() };
        if let Ok(data) = std::fs::read(&cache.file) {
            match parse(&data, &cache.root) {
                Some(entries) => {
                    cache.data = data;
                    cache.entries = entries;
                }
                None => tracing::debug!("ignoring the unreadable lowering cache {}", cache.file.display()),
            }
        }
        cache
    }

    /// Number of files in the bundle.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The encoded entry of `rel` when the file on disk still matches `stat` and the entry is
    /// intact (its checksum matches).
    pub fn bytes(&self, rel: &str, stat: Stat) -> Option<&[u8]> {
        let slot = self.entries.get(rel)?;
        let bytes = &self.data[slot.range.clone()];
        (slot.stat == stat && checksum(bytes) == slot.checksum).then_some(bytes)
    }

    /// The lowered file `rel` when the file on disk still matches `stat` and the entry decodes.
    pub fn get(&self, rel: &str, stat: Stat) -> Option<FileIr> {
        let ir = decode(self.bytes(rel, stat)?)?;
        (ir.file == rel).then_some(ir)
    }

    /// Replaces the bundle with `entries` (files that no longer exist are dropped with it).
    /// Best effort: a failure is logged and the next open lowers again.
    pub fn save<'a>(&self, entries: impl IntoIterator<Item = Entry<'a>>) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let racy = u64::try_from(now.saturating_sub(RACY).as_nanos()).unwrap_or(u64::MAX);
        let entries = entries.into_iter().filter(|e| e.stat.mtime < racy);
        let out = serialize(&self.root, entries, self.data.len());
        if let Err(e) = write_atomically(&self.file, &out) {
            tracing::warn!("cannot write the lowering cache {}: {e}", self.file.display());
        }
    }
}

/// A bundle of `entries` for `root` (what [`parse`] reads).
fn serialize<'a>(root: &str, entries: impl IntoIterator<Item = Entry<'a>>, capacity: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(capacity.max(1 << 16));
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(stamp());
    put_bytes(&mut out, root.as_bytes());
    let count_at = out.len();
    out.extend_from_slice(&0u32.to_le_bytes());
    let mut count = 0u32;
    for e in entries {
        put_bytes(&mut out, e.rel.as_bytes());
        out.extend_from_slice(&e.stat.size.to_le_bytes());
        out.extend_from_slice(&e.stat.mtime.to_le_bytes());
        out.extend_from_slice(&checksum(e.bytes));
        put_bytes(&mut out, e.bytes);
        count += 1;
    }
    out[count_at..count_at + 4].copy_from_slice(&count.to_le_bytes());
    out
}

/// LZ4 has no checksum of its own: a damaged entry could otherwise decode into wrong data.
fn checksum(bytes: &[u8]) -> [u8; 8] {
    let hash = blake3::hash(bytes);
    hash.as_bytes()[..8].try_into().unwrap_or_default()
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// Writes next to `file` and renames over it, so a reader never sees half a bundle.
fn write_atomically(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, file).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// The entries of a bundle, or `None` when it is not a complete bundle of this build for `root`.
fn parse(data: &[u8], root: &str) -> Option<HashMap<String, Slot>> {
    let mut r = Reader { data, at: 0 };
    if r.take(MAGIC.len())? != MAGIC || r.take(32)? != stamp() {
        return None;
    }
    let range = r.bytes()?;
    if &data[range] != root.as_bytes() {
        return None;
    }
    let count = r.u32()? as usize;
    let mut entries = HashMap::with_capacity(count.min(data.len() / 32));
    for _ in 0..count {
        let rel = std::str::from_utf8(&data[r.bytes()?]).ok()?.to_owned();
        let stat = Stat { size: r.u64()?, mtime: r.u64()? };
        let checksum = r.take(8)?.try_into().ok()?;
        let range = r.bytes()?;
        entries.insert(rel, Slot { stat, checksum, range });
    }
    (r.at == data.len()).then_some(entries)
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let range = self.at..self.at.checked_add(n)?;
        let slice = self.data.get(range.clone())?;
        self.at = range.end;
        Some(slice)
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }

    /// A length-prefixed byte string, as a range of the data.
    fn bytes(&mut self) -> Option<Range<usize>> {
        let len = self.u32()? as usize;
        let start = self.at;
        self.take(len)?;
        Some(start..start + len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nodex_lang_api::{LanguageFrontend, SourceFile};

    fn lowered() -> FileIr {
        let text = r#"
namespace App {
  /// Says hi.
  public class Greeter : IGreeter {
    private int count;
    public static string Hi(string name) { if (name == null) { return "?"; } count++; return "hi " + name; }
  }
}
"#;
        nodex_lang_csharp::CSharpFrontend.lower_file(&SourceFile { rel_path: "src/Greeter.cs".into(), text: text.into() })
    }

    #[test]
    fn a_lowered_file_reads_back_identical() {
        let ir = lowered();
        assert!(!ir.symbols.is_empty() && !ir.flows.is_empty());
        // Symbols leave default fields out: they must come back as defaults, in place.
        assert!(ir.symbols.iter().any(|s| s.parent.is_none()) && ir.symbols.iter().any(|s| s.doc.is_some()));
        assert_eq!(decode(&encode(&ir).unwrap()), Some(ir));
    }

    #[test]
    fn damaged_encodings_are_rejected_without_panicking() {
        let bytes = encode(&lowered()).unwrap();
        assert_eq!(decode(&[]), None);
        assert_eq!(decode(&[0xff; 4]), None);
        assert_eq!(decode(&[0xff, 0xff, 0xff, 0xff, 1, 2, 3]), None);
        for len in 0..bytes.len() {
            let _ = decode(&bytes[..len]);
        }
    }

    #[test]
    fn a_bundle_reads_back_and_any_truncation_is_refused() {
        let ir = lowered();
        let bytes = encode(&ir).unwrap();
        let stat = Stat { size: 10, mtime: 20 };
        let data = serialize("/root", [Entry { rel: "src/Greeter.cs", stat, bytes: &bytes }], 0);
        let entries = parse(&data, "/root").expect("a complete bundle parses");
        assert_eq!(entries.len(), 1);
        let cache = LowerCache { file: PathBuf::new(), root: "/root".into(), data: data.clone(), entries };
        assert_eq!(cache.get("src/Greeter.cs", stat), Some(ir));
        assert_eq!(cache.get("src/Greeter.cs", Stat { size: 10, mtime: 21 }), None, "a changed file misses");
        assert_eq!(cache.get("src/Other.cs", stat), None);
        assert!(parse(&data, "/other").is_none(), "another root's bundle is refused");
        for len in 0..data.len() {
            assert!(parse(&data[..len], "/root").is_none(), "truncated at {len}");
        }
    }

    #[test]
    fn a_damaged_entry_misses() {
        let bytes = encode(&lowered()).unwrap();
        let stat = Stat { size: 10, mtime: 20 };
        let mut data = serialize("/root", [Entry { rel: "src/Greeter.cs", stat, bytes: &bytes }], 0);
        let last = data.len() - 1;
        data[last] ^= 0x55;
        let entries = parse(&data, "/root").unwrap();
        let cache = LowerCache { file: PathBuf::new(), root: "/root".into(), data, entries };
        assert_eq!(cache.get("src/Greeter.cs", stat), None);
    }
}
