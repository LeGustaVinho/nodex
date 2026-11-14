//! Live traces: an instrumented copy of the Cargo project.
//!
//! The project is copied to `<root>/.nodex/run/rust/src` (never touching the user's files or
//! `target/`). In the copy, every function with a Blueprint gets probes from the
//! `nodex-trace-rt` runtime crate (`sidecars/rust/nodex-trace-rt`), inserted as text, so the
//! trace map's points keep the byte ranges of the original files:
//!
//! - start of the body: `let __nx_g = enter(M);` (its drop records the end, also on `?`,
//!   early returns and panics) and the arguments;
//! - before statements without outer attributes: `step(M, P)`; after a `let x = …` or `x = …`: the new value;
//! - the function's tail expression: its value is recorded as the result;
//! - `e.await` becomes `traced(R, e).await` (suspensions end the frame; resuming enters `R`,
//!   the function's `resume` entry, like .NET and JavaScript).
//!
//! The copy depends on the runtime by path and is built with `cargo build` into
//! `<root>/.nodex/run/rust/target` (kept for incremental builds); lints are capped so the
//! inserted code never fails a `#![deny(warnings)]`.

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use nodex_lang_api::{RunCommand, RunEnv, RunKind, TraceInstrumenter, kit::LineIndex};
use serde_json::{Value, json};
use tree_sitter::Node;

use crate::structure::{BodyJob, Collector};
use crate::syntax::{Src, has_semicolon, is_comment, is_item, named_children, unparen};

const SKIP_DIRS: &[&str] = &["target", ".git", ".nodex", "node_modules"];
const RT: &str = "::nodex_trace_rt";

/// Builds and runs an instrumented copy of a Cargo binary.
pub struct RustInstrumenter;

/// The `nodex-trace-rt` crate: `NODEX_RUST_TRACE_RT` (its folder or `Cargo.toml`), next to the
/// executable, or in the repository.
pub fn runtime_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("NODEX_RUST_TRACE_RT").map(PathBuf::from) {
        let dir = if p.is_dir() { p } else { p.parent()?.to_path_buf() };
        return dir.join("Cargo.toml").is_file().then_some(dir);
    }
    let manifest = nodex_lang_api::sidecar::locate(
        "NODEX_RUST_TRACE_RT_MANIFEST",
        &["sidecars/rust/nodex-trace-rt/Cargo.toml", "nodex-trace-rt/Cargo.toml"],
        &["sidecars/rust/nodex-trace-rt/Cargo.toml"],
    )?;
    manifest.parent().map(Path::to_path_buf)
}

impl TraceInstrumenter for RustInstrumenter {
    fn prepare(&self, root: &Path, out_dir: &Path, _env: &RunEnv, log: &mut dyn FnMut(&str)) -> Result<RunCommand, String> {
        let runtime = runtime_dir().ok_or("the Rust trace runtime (sidecars/rust/nodex-trace-rt) was not found; set NODEX_RUST_TRACE_RT")?;
        let packages = crate::project::cargo_packages(root);
        let (package, (bin, _)) = packages
            .iter()
            .find_map(|p| p.bins.first().map(|b| (p, b.clone())))
            .ok_or("no binary to run: add src/main.rs, a src/bin target, or a [[bin]] target")?;
        let work = out_dir.join("rust");
        let copy = work.join("src");
        log("Copying the project …");
        if copy.exists() {
            std::fs::remove_dir_all(&copy).map_err(|e| format!("cannot clear {}: {e}", copy.display()))?;
        }
        copy_tree(root, &copy).map_err(|e| format!("cannot copy the project: {e}"))?;

        log("Instrumenting …");
        let mut methods = Vec::new();
        let mut next_id = 1;
        let mut instrumented_files = 0;
        for p in &packages {
            let Some(dir) = p.manifest.parent() else { continue };
            if is_proc_macro(&p.manifest) {
                continue;
            }
            let rel_dir = dir.strip_prefix(root).unwrap_or(Path::new(""));
            let mut files = Vec::new();
            rust_files(&dir.join("src"), &mut files);
            for file in files {
                let rel = file.strip_prefix(root).unwrap_or(&file).to_string_lossy().replace('\\', "/");
                let Ok(text) = std::fs::read_to_string(&file) else { continue };
                if let Some(out) = instrument_source(&rel, &text, &mut next_id, &mut methods) {
                    std::fs::write(copy.join(&rel), out).map_err(|e| e.to_string())?;
                    instrumented_files += 1;
                }
            }
            add_runtime_dependency(&copy.join(rel_dir).join("Cargo.toml"), &runtime)?;
        }
        log(&format!("{instrumented_files} files, {} functions instrumented", methods.len()));
        let map = work.join("nodex-trace-map.json");
        std::fs::write(&map, serde_json::to_string(&json!({ "version": 1, "methods": methods })).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

        let rel_manifest = package.manifest.strip_prefix(root).unwrap_or(&package.manifest);
        let target = work.join("target");
        log(&format!("cargo build --bin {bin} …"));
        let mut child = nodex_lang_api::process::command("cargo")
            .arg("build")
            .arg("--manifest-path")
            .arg(copy.join(rel_manifest))
            .arg("--bin")
            .arg(&bin)
            .arg("--target-dir")
            .arg(&target)
            .arg("--color")
            .arg("never")
            .env("RUSTFLAGS", "--cap-lints allow")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start cargo ({e}); is Rust installed?"))?;
        let stdout = child.stdout.take();
        let out_thread = std::thread::spawn(move || {
            stdout.map(|s| BufReader::new(s).lines().map_while(Result::ok).collect::<Vec<_>>()).unwrap_or_default()
        });
        let mut errors = Vec::new();
        if let Some(stderr) = child.stderr.take() {
            for l in BufReader::new(stderr).lines().map_while(Result::ok) {
                log(&l);
                if l.starts_with("error") {
                    errors.push(l);
                }
            }
        }
        let _ = out_thread.join();
        let status = child.wait().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(errors.first().cloned().unwrap_or_else(|| "cargo build failed".into()));
        }
        let exe = target.join("debug").join(format!("{bin}{}", std::env::consts::EXE_SUFFIX));
        Ok(RunCommand {
            program: exe.to_string_lossy().into_owned(),
            args: Vec::new(),
            cwd: root.to_path_buf(),
            trace_map: map,
            description: format!("{bin} (instrumented debug build)"),
            kind: RunKind::Program,
            env: Vec::new(),
        })
    }
}

fn is_proc_macro(manifest: &Path) -> bool {
    std::fs::read_to_string(manifest)
        .ok()
        .and_then(|t| toml::from_str::<toml::Table>(&t).ok())
        .and_then(|t| t.get("lib").and_then(|l| l.get("proc-macro")).and_then(toml::Value::as_bool))
        .unwrap_or(false)
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let path = entry.path();
        if path.is_dir() {
            if SKIP_DIRS.iter().any(|s| name == *s) {
                continue;
            }
            copy_tree(&path, &to.join(&name))?;
        } else {
            std::fs::copy(&path, to.join(&name))?;
        }
    }
    Ok(())
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Adds `nodex-trace-rt = { path = … }` to a copied manifest's `[dependencies]`.
fn add_runtime_dependency(manifest: &Path, runtime: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(manifest).map_err(|e| format!("cannot read {}: {e}", manifest.display()))?;
    if text.contains("nodex-trace-rt") {
        return Ok(());
    }
    let path = runtime.to_string_lossy().replace('\\', "/");
    let dep = format!("nodex-trace-rt = {{ path = \"{path}\" }}\n");
    let mut out = String::new();
    let mut done = false;
    for line in text.split_inclusive('\n') {
        out.push_str(line);
        if !done && line.trim() == "[dependencies]" {
            if !line.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&dep);
            done = true;
        }
    }
    if !done {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("\n[dependencies]\n");
        out.push_str(&dep);
    }
    std::fs::write(manifest, out).map_err(|e| e.to_string())
}

/// Text inserted at a byte offset; edits at the same offset keep their creation order.
struct Edit {
    at: usize,
    seq: usize,
    text: String,
}

struct Method {
    id: i32,
    resume: Option<i32>,
    points: Vec<(usize, usize)>,
    locals: BTreeMap<String, i32>,
}

struct Rewriter<'a> {
    src: Src<'a>,
    edits: Vec<Edit>,
}

impl Rewriter<'_> {
    fn insert(&mut self, at: usize, text: String) {
        let seq = self.edits.len();
        self.edits.push(Edit { at, seq, text });
    }

    fn point(&self, m: &mut Method, node: Node) -> i32 {
        m.points.push((node.start_byte(), node.end_byte()));
        (m.points.len() - 1) as i32
    }

    fn slot(m: &mut Method, name: &str) -> i32 {
        let next = m.locals.len() as i32;
        *m.locals.entry(name.to_owned()).or_insert(next)
    }

    /// A block's statements; `ret` records the tail expression as the function's result.
    fn block(&mut self, m: &mut Method, block: Node, ret: bool) {
        let items: Vec<Node> = named_children(block).into_iter().filter(|c| !is_comment(c.kind()) && !is_item(c.kind())).collect();
        let count = items.len();
        for (i, s) in items.into_iter().enumerate() {
            // A probe before an attributed statement would take its attribute.
            let mut previous = s.prev_named_sibling();
            while previous.is_some_and(|n| is_comment(n.kind())) {
                previous = previous.and_then(|n| n.prev_named_sibling());
            }
            if previous.is_some_and(|n| n.kind() == "attribute_item") {
                continue;
            }
            let last = i + 1 == count;
            let tail = last
                && match s.kind() {
                    "expression_statement" => !has_semicolon(self.src, s),
                    "let_declaration" => false,
                    _ => true,
                };
            let p = self.point(m, s);
            let id = m.id;
            if tail && ret {
                self.insert(s.start_byte(), format!("{RT}::step({id}, {p}); {RT}::returning("));
                self.descend(m, s);
                self.insert(s.end_byte(), format!(", |__nx_r| {{ {RT}::nx_ret!({id}, *__nx_r); }})"));
                continue;
            }
            self.insert(s.start_byte(), format!("{RT}::step({id}, {p}); "));
            self.descend(m, s);
            // The value a local now holds.
            let target = match s.kind() {
                "let_declaration" if s.child_by_field_name("value").is_some() && s.child_by_field_name("alternative").is_none() => {
                    s.child_by_field_name("pattern").and_then(simple_binding).map(|n| self.src.text(n).to_owned())
                }
                "expression_statement" if has_semicolon(self.src, s) => named_children(s)
                    .into_iter()
                    .next()
                    .map(unparen)
                    .filter(|e| matches!(e.kind(), "assignment_expression" | "compound_assignment_expr"))
                    .and_then(|e| e.child_by_field_name("left"))
                    .filter(|l| l.kind() == "identifier")
                    .map(|l| self.src.text(l).to_owned()),
                _ => None,
            };
            if let Some(name) = target {
                let slot = Self::slot(m, &name);
                self.insert(s.end_byte(), format!(" {RT}::nx_local!({id}, {slot}, {name});"));
            }
        }
    }

    /// Nested blocks of a statement (branches, loop bodies, match arms), not other functions.
    fn descend(&mut self, m: &mut Method, node: Node) {
        for c in named_children(node) {
            match c.kind() {
                "closure_expression" | "async_block" | "function_item" | "const_block" | "gen_block" | "macro_invocation"
                | "impl_item" | "trait_item" | "mod_item" | "struct_item" | "enum_item" => {}
                "block" => self.block(m, c, false),
                "match_arm" => {
                    if let Some(p) = c.child_by_field_name("pattern") {
                        self.descend(m, p);
                    }
                    match c.child_by_field_name("value") {
                        Some(v) if v.kind() == "block" => self.block(m, v, false),
                        Some(v) => {
                            let p = self.point(m, v);
                            let id = m.id;
                            self.insert(v.start_byte(), format!("{{ {RT}::step({id}, {p}); "));
                            self.descend_expr(m, v);
                            self.insert(v.end_byte(), " }".into());
                        }
                        None => {}
                    }
                }
                _ => self.descend_expr(m, c),
            }
        }
    }

    fn descend_expr(&mut self, m: &mut Method, e: Node) {
        if e.kind() == "await_expression"
            && let Some(r) = m.resume
            && let Some(inner) = named_children(e).into_iter().next()
        {
            self.insert(inner.start_byte(), format!("{RT}::traced({r}, "));
            self.descend_expr(m, inner);
            self.insert(inner.end_byte(), ")".into());
            return;
        }
        match e.kind() {
            "closure_expression" | "async_block" | "function_item" | "const_block" | "gen_block" | "macro_invocation" => {}
            "block" => self.block(m, e, false),
            _ => self.descend(m, e),
        }
    }
}

/// `x` or `mut x`.
fn simple_binding(p: Node) -> Option<Node> {
    match p.kind() {
        "identifier" => Some(p),
        "mut_pattern" => named_children(p).into_iter().last().filter(|n| n.kind() == "identifier"),
        _ => None,
    }
}

/// The instrumented text of one file, adding its methods to `methods`; `None` when the file is
/// left as it is (no functions, or syntax errors).
pub fn instrument_source(rel: &str, text: &str, next_id: &mut i32, methods: &mut Vec<Value>) -> Option<String> {
    let tree = crate::parse(text)?;
    let root = tree.root_node();
    if root.has_error() {
        return None;
    }
    let index = LineIndex::new(text);
    let src = Src { text, file: rel, index: &index };
    let mut collector = Collector::new(src);
    collector.source_file(root);
    let collected = collector.finish();
    let mut rw = Rewriter { src, edits: Vec::new() };
    let mut entries: Vec<(String, Method, Vec<String>)> = Vec::new();
    for job in &collected.bodies {
        if !instrumentable(job) {
            continue;
        }
        let id = *next_id;
        *next_id += 1;
        let resume = job.flavors.iter().any(|f| f == "async").then(|| {
            let r = *next_id;
            *next_id += 1;
            r
        });
        let mut m = Method { id, resume, points: Vec::new(), locals: BTreeMap::new() };
        // Point 0: the body's opening brace (counts for the function, not a statement).
        m.points.push((job.body.start_byte(), job.body.start_byte() + 1));
        let mut head = format!(" let __nx_g = {RT}::enter({id}); {RT}::step({id}, 0);");
        for (i, (name, _)) in job.params.iter().enumerate() {
            if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') && name != "_" {
                head.push_str(&format!(" {RT}::nx_arg!({id}, {i}, {name});"));
            }
        }
        let entry = named_children(job.body).into_iter()
            .find(|node| !is_comment(node.kind()) && node.kind() != "inner_attribute_item")
            .map_or(job.body.end_byte() - 1, |node| node.start_byte());
        rw.insert(entry, head);
        rw.block(&mut m, job.body, job.returns_value);
        let params = job.params.iter().map(|(n, _)| n.clone()).collect();
        entries.push((job.symbol.clone(), m, params));
    }
    if entries.is_empty() {
        return None;
    }
    for (name, m, params) in entries {
        let points: Vec<Value> = m.points.iter().map(|(s, e)| json!({ "file": rel, "start": s, "end": e })).collect();
        let locals: HashMap<String, String> = m.locals.iter().map(|(n, s)| (s.to_string(), n.clone())).collect();
        methods.push(json!({ "id": m.id, "name": name, "kind": "method", "points": points, "locals": locals, "params": params }));
        if let Some(r) = m.resume {
            methods.push(json!({ "id": r, "name": format!("{name}::resume"), "kind": "resume", "points": [points[0].clone()], "locals": locals, "params": [] }));
        }
    }
    let mut edits = rw.edits;
    edits.sort_by_key(|e| (e.at, e.seq));
    let mut out = String::with_capacity(text.len() + edits.len() * 40);
    let mut pos = 0;
    for e in edits {
        out.push_str(&text[pos..e.at]);
        out.push_str(&e.text);
        pos = e.at;
    }
    out.push_str(&text[pos..]);
    Some(out)
}

/// Bodies that can hold probes: not `const fn` (probes are not const) and with a block body.
fn instrumentable(job: &BodyJob) -> bool {
    job.body.kind() == "block" && !job.flavors.iter().any(|f| f == "const")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probes_go_around_statements_and_keep_original_points() {
        let text = "fn total(items: &[i32]) -> i32 {\n    let mut sum = 0;\n    for x in items {\n        sum += x;\n    }\n    sum\n}\n";
        let mut methods = Vec::new();
        let mut id = 1;
        let out = instrument_source("src/lib.rs", text, &mut id, &mut methods).expect("instrumented");
        assert!(out.contains("::nodex_trace_rt::enter(1)"), "{out}");
        assert!(out.contains("::nodex_trace_rt::nx_arg!(1, 0, items)"), "{out}");
        assert!(out.contains("::nodex_trace_rt::nx_local!(1, 0, sum)"), "{out}");
        assert!(out.contains("::nodex_trace_rt::returning(sum, |__nx_r| { ::nodex_trace_rt::nx_ret!(1, *__nx_r); })"), "{out}");
        let points = methods[0]["points"].as_array().unwrap();
        // Brace, `let`, `for`, `sum += x;`, tail `sum`: points read the original text.
        let texts: Vec<&str> = points.iter().map(|p| &text[p["start"].as_u64().unwrap() as usize..p["end"].as_u64().unwrap() as usize]).collect();
        assert_eq!(texts, ["{", "let mut sum = 0;", "for x in items {\n        sum += x;\n    }", "sum += x;", "sum"]);
        assert!(crate::parse(&out).is_some_and(|t| !t.root_node().has_error()), "{out}");
    }

    #[test]
    fn awaits_and_match_arms() {
        let text = "async fn f(x: Option<u32>) -> u32 {\n    let v = g().await;\n    match x {\n        Some(n) => n + v,\n        None => 0,\n    }\n}\nasync fn g() -> u32 { 1 }\n";
        let mut methods = Vec::new();
        let mut id = 1;
        let out = instrument_source("src/lib.rs", text, &mut id, &mut methods).expect("instrumented");
        assert!(out.contains("::nodex_trace_rt::traced(2, g()).await"), "{out}");
        assert!(out.contains("{ ::nodex_trace_rt::step(1, 3); n + v }"), "{out}");
        assert!(methods.iter().any(|m| m["kind"] == "resume" && m["id"] == 2));
        assert!(crate::parse(&out).is_some_and(|t| !t.root_node().has_error()), "{out}");
    }
}
