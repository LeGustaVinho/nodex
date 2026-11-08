//! Lowers every Rust file under one or more folders and aggregates what the frontend does not
//! map: files with parse errors (with the grammar's first failure), `Opaque` statements and
//! expressions, and `Macro` statements by macro name.
//!
//! Usage: cargo run --release -p nodex-lang-rust --example rs_corpus -- <folder>... [--show N]
//! The local cargo registry (`~/.cargo/registry/src`) is a good corpus.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nodex_ir::{Concept, Expr};

const SKIP: &[&str] = &["target", ".git", ".nodex"];
/// Same limit as `nodex-core` (generated code is skipped).
const MAX_BYTES: u64 = 2 * 1024 * 1024;

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_owned();
        if p.is_dir() {
            if !SKIP.contains(&name.as_str()) {
                walk(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "rs") && e.metadata().is_ok_and(|m| m.len() <= MAX_BYTES) {
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
    let roots: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|(i, a)| !a.starts_with("--") && !(*i > 0 && args[i - 1] == "--show"))
        .map(|(_, a)| a)
        .collect();
    let started = std::time::Instant::now();
    let mut files = Vec::new();
    for r in &roots {
        walk(Path::new(r), &mut files);
    }
    let mut parse_errors: Vec<(String, usize, String)> = Vec::new();
    let mut opaque_stmts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut opaque_exprs: BTreeMap<String, usize> = BTreeMap::new();
    let mut macros: BTreeMap<String, usize> = BTreeMap::new();
    let (mut statements, mut flows, mut symbols, mut ops, mut calls) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        let rel = f.to_string_lossy().replace('\\', "/");
        let rel = rel.rsplit("/src/").next().map(|t| format!("src/{t}")).unwrap_or(rel);
        let ir = nodex_lang_rust::lower_source(&rel, &text);
        if !ir.diagnostics.is_empty() {
            let first = nodex_lang_rust::remaining_parse_errors(&text).into_iter().next().unwrap_or((0, String::new()));
            parse_errors.push((f.display().to_string(), first.0, first.1));
        }
        symbols += ir.symbols.len();
        for flow in &ir.flows {
            flows += 1;
            for n in &flow.nodes {
                statements += 1;
                if n.concept == Concept::Opaque {
                    let kind = n.flavors.iter().find(|x| x.starts_with("syntax:")).cloned().unwrap_or_default();
                    opaque_stmts.entry(kind).or_default().push(n.code.chars().take(100).collect());
                }
                if n.concept == Concept::Macro {
                    let name = n.code.split('!').next().unwrap_or("").rsplit("::").next().unwrap_or("").to_owned();
                    *macros.entry(name).or_default() += 1;
                }
                if let Some(e) = &n.expr {
                    let mut found = Vec::new();
                    opaque(e, &mut found);
                    for k in found {
                        *opaque_exprs.entry(k).or_default() += 1;
                    }
                    let mut stack = vec![e];
                    while let Some(x) = stack.pop() {
                        if x.op.is_some() {
                            ops += 1;
                        }
                        if x.concept == Concept::Call {
                            calls += 1;
                        }
                        stack.extend(x.children.iter());
                    }
                }
            }
        }
    }
    let opaque_count: usize = opaque_stmts.values().map(Vec::len).sum();
    println!(
        "{} files, {} symbols, {} flows, {} statements in {:.1?}",
        files.len(),
        symbols,
        flows,
        statements,
        started.elapsed()
    );
    println!(
        "files with parse errors: {} ({:.2}%)",
        parse_errors.len(),
        100.0 * parse_errors.len() as f64 / files.len().max(1) as f64
    );
    for (file, line, code) in parse_errors.iter().take(show * 4) {
        println!("  {file}:{line}  {code}");
    }
    println!("opaque statements: {opaque_count} ({:.3}%)", 100.0 * opaque_count as f64 / statements.max(1) as f64);
    for (kind, codes) in &opaque_stmts {
        println!("  {kind}: {}", codes.len());
        for c in codes.iter().take(show) {
            println!("      {c}");
        }
    }
    println!("opaque expressions: {}", opaque_exprs.values().sum::<usize>());
    let mut by_count: Vec<_> = opaque_exprs.into_iter().collect();
    by_count.sort_by_key(|e| std::cmp::Reverse(e.1));
    for (k, n) in by_count.iter().take(20) {
        println!("  {k}: {n}");
    }
    println!("operations recognized in expressions: {ops} (calls left as calls: {calls})");
    let mut m: Vec<_> = macros.into_iter().collect();
    m.sort_by_key(|e| std::cmp::Reverse(e.1));
    println!("project/crate macros as Macro nodes (top): {}", m.iter().take(12).map(|(k, n)| format!("{k}!×{n}")).collect::<Vec<_>>().join(", "));
}
