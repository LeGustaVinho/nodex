//! Lowers every TypeScript/JavaScript file under a folder and aggregates what the frontend does not map.
//! Usage: cargo run --release -p nodex-lang-typescript --example ts_corpus -- <folder> [--show N]

use std::collections::BTreeMap;
use std::path::Path;

use nodex_ir::{Concept, Expr};

const SKIP: &[&str] = &["node_modules", ".git", "dist", "build", "out", "coverage", ".next", ".turbo", "release", "playwright-report", "test-results"];

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if !p.file_name().and_then(|n| n.to_str()).is_some_and(|n| SKIP.contains(&n)) {
                walk(&p, out);
            }
        } else if p.extension().is_some_and(|x| ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"].iter().any(|e| x == *e) && !p.to_string_lossy().ends_with(".min.js")) {
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
    let root = Path::new(&args[0]);
    let show: usize = args.iter().position(|a| a == "--show").and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(3);
    let mut files = Vec::new();
    walk(root, &mut files);
    let (mut statements, mut diag_files, mut merged) = (0usize, 0usize, 0usize);
    let mut stmt_kinds: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut expr_kinds: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut diag_examples = Vec::new();
    for f in &files {
        let Ok(bytes) = std::fs::read(f) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        let rel = f.strip_prefix(root).unwrap_or(f).to_string_lossy().replace('\\', "/");
        let ir = nodex_lang_typescript::lower_source(&rel, &text);
        let problems: Vec<_> = ir.diagnostics.iter().filter(|d| d.severity != nodex_ir::Severity::Info).collect();
        merged += ir.diagnostics.len() - problems.len();
        if !problems.is_empty() {
            diag_files += 1;
            if let Some(s) = problems[0].span.as_ref() {
                let line = text.lines().nth(s.start_line as usize - 1).unwrap_or("").trim().chars().take(100).collect::<String>();
                diag_examples.push(format!("{rel}:{}: {line}", s.start_line));
            }
        }
        for flow in &ir.flows {
            for n in &flow.nodes {
                statements += 1;
                if n.concept == Concept::Opaque {
                    let kind = n.flavors.iter().find(|x| x.starts_with("syntax:")).cloned().unwrap_or_else(|| "syntax:?".into());
                    stmt_kinds.entry(kind).or_default().push(format!("{rel}:{}: {}", n.span.start_line, n.code.chars().take(90).collect::<String>()));
                }
                if let Some(e) = &n.expr {
                    let mut kinds = Vec::new();
                    opaque(e, &mut kinds);
                    for k in kinds {
                        expr_kinds.entry(k).or_default().push(format!("{rel}:{}: {}", n.span.start_line, n.code.chars().take(90).collect::<String>()));
                    }
                }
            }
        }
    }
    println!("files {} | statements {} | files with parse errors {} | merged #if groups {}", files.len(), statements, diag_files, merged);
    for d in diag_examples.iter().take(show * 4) {
        println!("  DIAG {d}");
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
}
