//! Shared helpers for the knowledge passes of the language frontends: finding the symbol that
//! owns a piece of code, reading string literals, and the side-effect tables ported from
//! CodeGraph (`crates/core/src/extract/capabilities.rs`).

use nodex_ir::{Symbol, SymbolId, SymbolKind};

struct Owner {
    start: u32,
    end: u32,
    id: SymbolId,
    kind: SymbolKind,
    has_flow: bool,
}

/// Finds the symbol declared around a byte offset of one file.
pub struct Owners {
    entries: Vec<Owner>,
}

impl Owners {
    pub fn new(symbols: &[Symbol], file: &str) -> Self {
        let mut entries = Vec::new();
        for s in symbols.iter().filter(|s| !s.external) {
            for o in s.origins.iter().filter(|o| o.file == file) {
                entries.push(Owner { start: o.start_byte, end: o.end_byte, id: s.id.clone(), kind: s.kind, has_flow: s.has_flow });
            }
        }
        Self { entries }
    }

    fn innermost(&self, byte: usize, filter: impl Fn(&Owner) -> bool) -> Option<&Owner> {
        let byte = byte as u32;
        self.entries.iter().filter(|e| e.start <= byte && byte < e.end && filter(e)).min_by_key(|e| e.end - e.start)
    }

    /// The function whose body contains `byte` (a symbol with a flow graph), else any symbol.
    pub fn function_at(&self, byte: usize) -> Option<&str> {
        self.innermost(byte, |e| e.has_flow).or_else(|| self.innermost(byte, |_| true)).map(|e| e.id.as_str())
    }

    /// The symbol declared exactly by the node starting at `byte` with the given kind filter.
    pub fn declared_at(&self, start: usize, end: usize, filter: impl Fn(SymbolKind) -> bool) -> Option<&str> {
        self.entries
            .iter()
            .filter(|e| e.start == start as u32 && e.end == end as u32 && filter(e.kind))
            .map(|e| e.id.as_str())
            .next()
    }

    /// The innermost type around `byte`.
    pub fn type_at(&self, byte: usize) -> Option<&str> {
        self.innermost(byte, |e| e.kind.is_type()).map(|e| e.id.as_str())
    }
}

/// The text of a string literal without quotes and prefixes (`"x"`, `@"x"`, `'x'`, `` `x` ``,
/// `$"a{b}"`, `r#"x"#`). Interpolations stay as written (`a{b}`, `a${b}`); `None` when `text`
/// is not a string literal.
pub fn unquote(text: &str) -> Option<String> {
    let t = text.trim();
    let t = t.trim_start_matches(['$', '@', 'u', 'U', 'L', 'b']);
    // Rust raw strings: r"..", r#".."#.
    let t = t.strip_prefix('r').map(|r| r.trim_matches('#')).unwrap_or(t);
    for q in ['"', '\'', '`'] {
        if t.len() >= 2 && t.starts_with(q) && t.ends_with(q) {
            let inner = &t[1..t.len() - 1];
            // C# raw strings `"""x"""`.
            return Some(inner.trim_matches('"').to_owned());
        }
    }
    None
}

/// HTTP method of a client call name (`GetAsync` → `GET`, `postForEntity` → `POST`).
pub fn http_method_of(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    for (prefix, method) in [("get", "GET"), ("post", "POST"), ("put", "PUT"), ("delete", "DELETE"), ("patch", "PATCH"), ("head", "HEAD")] {
        if n.starts_with(prefix) {
            return Some(method);
        }
    }
    None
}

pub const DB_READ_CALLS: &[&str] = &[
    "ToList", "ToListAsync", "ToArray", "ToArrayAsync", "FirstOrDefault", "FirstOrDefaultAsync", "SingleOrDefault",
    "SingleOrDefaultAsync", "First", "FirstAsync", "Find", "FindAsync", "AnyAsync", "CountAsync", "Include", "AsNoTracking",
    "FromSqlRaw", "findMany", "findUnique", "findFirst", "findOne", "findById", "findAll", "find", "findOneBy", "findBy",
    "aggregate", "query", "QueryAsync", "QueryFirstOrDefaultAsync",
];

pub const DB_WRITE_CALLS: &[&str] = &[
    "SaveChanges", "SaveChangesAsync", "ExecuteSqlRaw", "ExecuteSqlRawAsync", "ExecuteAsync", "BulkInsert", "ExecuteDelete",
    "ExecuteDeleteAsync", "ExecuteUpdate", "ExecuteUpdateAsync",
];

pub const FILE_CALLS: &[&str] = &[
    "ReadAllText", "ReadAllTextAsync", "WriteAllText", "WriteAllTextAsync", "ReadAllBytes", "WriteAllBytes", "AppendAllText",
    "readFile", "readFileSync", "writeFile", "writeFileSync", "appendFile", "createReadStream", "createWriteStream", "unlink",
    "unlinkSync", "mkdir", "read_to_string", "write",
];

/// Attributes or decorators that require an authenticated caller.
pub const AUTH_MARKERS: &[&str] = &["Authorize", "UseGuards", "AuthGuard", "Roles", "RequireAuthorization"];

/// Side effects of a call, from its name and its (lower-cased) receiver: `DB_READ`, `DB_WRITE`,
/// `FILE_IO`, `CACHE`, `LOGGING`, `LOCAL_STORAGE`, `EVENT_EMIT`. HTTP calls are recorded as facts.
pub fn call_capabilities(name: &str, receiver: &str) -> Vec<&'static str> {
    let recv = receiver.to_ascii_lowercase();
    let last = recv.rsplit(['.', ':']).next().unwrap_or("").trim_start_matches('_').to_owned();
    let mut tags = Vec::new();
    let dbish = ["db", "context", "ctx", "prisma", "repo", "session", "collection", "query"].iter().any(|w| recv.contains(w));
    if DB_READ_CALLS.contains(&name) && dbish {
        tags.push("DB_READ");
    }
    if DB_WRITE_CALLS.contains(&name) && (dbish || name.starts_with("SaveChanges")) {
        tags.push("DB_WRITE");
    }
    let fileish = matches!(last.as_str(), "file" | "fs" | "directory" | "promises") || recv.ends_with("::fs");
    if FILE_CALLS.contains(&name) && fileish {
        tags.push("FILE_IO");
    }
    if last == "playerprefs" || last == "editorprefs" || last == "localstorage" || last == "sessionstorage" {
        tags.push("LOCAL_STORAGE");
    }
    if recv.contains("cache") || recv.contains("redis") {
        tags.push("CACHE");
    }
    if recv.contains("logger") || last == "log" || last == "console" || last == "debug" && name.starts_with("Log") {
        tags.push("LOGGING");
    }
    if matches!(name, "publish" | "emit" | "Publish" | "PublishAsync" | "dispatch") {
        tags.push("EVENT_EMIT");
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unquotes_literals_of_every_language() {
        assert_eq!(unquote("\"/api/x\"").as_deref(), Some("/api/x"));
        assert_eq!(unquote("@\"C:\\x\"").as_deref(), Some("C:\\x"));
        assert_eq!(unquote("$\"/api/orders/{id}\"").as_deref(), Some("/api/orders/{id}"));
        assert_eq!(unquote("'/x'").as_deref(), Some("/x"));
        assert_eq!(unquote("`/api/${id}`").as_deref(), Some("/api/${id}"));
        assert_eq!(unquote("r#\"x\"#").as_deref(), Some("x"));
        assert_eq!(unquote("url"), None);
    }

    #[test]
    fn capabilities_come_from_names_and_receivers() {
        assert_eq!(call_capabilities("ToListAsync", "_db.Orders"), vec!["DB_READ"]);
        assert_eq!(call_capabilities("SaveChangesAsync", "_db"), vec!["DB_WRITE"]);
        assert_eq!(call_capabilities("WriteLine", "Console"), vec!["LOGGING"]);
        assert_eq!(call_capabilities("readFileSync", "fs"), vec!["FILE_IO"]);
        assert!(call_capabilities("Add", "list").is_empty());
        assert_eq!(http_method_of("PostAsJsonAsync"), Some("POST"));
    }
}
