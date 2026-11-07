//! Lowers every TypeScript/JavaScript file under one or more folders and aggregates what the
//! frontend does not map: files with parse errors (with the grammar's first failure),
//! `Opaque` statements and expressions, and statements kept only as a bare value.
//!
//! Usage: cargo run --release -p nodex-lang-typescript --example ts_corpus -- <folder>... [--show N] [--all]
//! `--all` also reads `node_modules`, `dist` and `build` (published packages are good corpora).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nodex_ir::{Concept, Expr};

const SKIP: &[&str] = &["node_modules", ".git", "dist", "build", "out", "coverage", ".next", ".turbo", "release", "playwright-report", "test-results"];
const EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];
/// Same limit as `nodex-core` (bundles and generated code are skipped).
const MAX_BYTES: u64 = 2 * 1024 * 1024;

fn walk(dir: &Path, all: bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_owned();
        if p.is_dir() {
            if name == ".git" || (!all && SKIP.contains(&name.as_str())) {
                continue;
            }
            walk(&p, all, out);
        } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| EXTENSIONS.contains(&x))
            && !name.ends_with(".min.js")
            && e.metadata().is_ok_and(|m| m.len() <= MAX_BYTES)
        {
            out.push(p);
        }
    }
}

fn opaque(e: &Expr, out: &mut Vec<String>) {
    if e.concept == Concept::Opaque {
        out.push(e.flavors.iter().find(|f| f.starts_with("syntax:")).cloned().unwrap_or_else(|| "syntax:?".into()));
    }
    for c in &e.children {
        opaque(c, out);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let show: usize = args.iter().position(|a| a == "--show").and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(3);
    let all = args.iter().any(|a| a == "--all");
    let roots: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| !a.starts_with("--") && !(*i > 0 && args[i - 1] == "--show"))
        .map(|(_, a)| a)
        .collect();
    let started = std::time::Instant::now();
    let mut files = Vec::new();
    for r in &roots {
        walk(Path::new(r), all, &mut files);
    }
    let (mut statements, mut diag_files, mut flows, mut symbols) = (0usize, 0usize, 0usize, 0usize);
    let mut stmt_kinds: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut expr_kinds: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut grammar: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut values: Vec<String> = Vec::new();
    for f in &files {
        let Ok(bytes) = std::fs::read(f) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        let rel = f.to_string_lossy().replace('\\', "/");
        let name = rel.rsplit('/').next().unwrap_or(&rel).to_owned();
        let ir = nodex_lang_typescript::lower_source(&name, &text);
        symbols += ir.symbols.len();
        flows += ir.flows.len();
        if ir.diagnostics.iter().any(|d| d.severity != nodex_ir::Severity::Info) {
            diag_files += 1;
            let first = nodex_lang_typescript::remaining_parse_errors(&name, &text).into_iter().next();
            let (line, snippet) = first.unwrap_or((0, "?".into()));
            let key: String = snippet.chars().take(40).collect();
            grammar.entry(key).or_default().push(format!("{rel}:{line}"));
        }
        for flow in &ir.flows {
            for n in &flow.nodes {
                statements += 1;
                let at = || format!("{rel}:{}: {}", n.span.start_line, n.code.chars().take(90).collect::<String>());
                if n.concept == Concept::Opaque {
                    let kind = n.flavors.iter().find(|x| x.starts_with("syntax:")).cloned().unwrap_or_else(|| "syntax:?".into());
                    stmt_kinds.entry(kind).or_default().push(at());
                }
                if n.flavors.iter().any(|x| x == "value") {
                    values.push(at());
                }
                if let Some(e) = &n.expr {
                    let mut kinds = Vec::new();
                    opaque(e, &mut kinds);
                    for k in kinds {
                        expr_kinds.entry(k).or_default().push(at());
                    }
                }
            }
        }
    }
    println!(
        "files {} | symbols {symbols} | flows {flows} | statements {statements} | files with parse errors {diag_files} | {:.1}s",
        files.len(),
        started.elapsed().as_secs_f64()
    );
    let mut g: Vec<_> = grammar.iter().collect();
    g.sort_by_key(|(_, l)| std::cmp::Reverse(l.len()));
    println!("GRAMMAR FAILURES (first failure per file)");
    for (k, list) in g.iter().take(30) {
        println!("  {:>5}  {k}", list.len());
        for ex in list.iter().take(show) {
            println!("           {ex}");
        }
    }
    for (title, map) in [("OPAQUE STATEMENTS", &stmt_kinds), ("OPAQUE EXPRESSIONS", &expr_kinds)] {
        println!("{title}");
        let mut v: Vec<_> = map.iter().collect();
        v.sort_by_key(|(_, l)| std::cmp::Reverse(l.len()));
        for (k, list) in v {
            println!("  {:>6}  {k}", list.len());
            for ex in list.iter().take(show) {
                println!("            {ex}");
            }
        }
    }
    println!("BARE VALUE STATEMENTS {}", values.len());
    for ex in values.iter().take(show * 3) {
        println!("            {ex}");
    }
}
