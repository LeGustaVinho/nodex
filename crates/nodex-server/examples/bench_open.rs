//! Times opening a project in-process, phase by phase, the way the server does it.
//!
//! ```text
//! cargo run --release -p nodex-server --example bench_open -- <folder> [--semantic] [--runs N]
//! ```
//!
//! (`cargo xtask bench-open …` runs the same thing.) Each run opens the folder from scratch,
//! builds the structure graph and the knowledge, and serializes both as `/api/structure` and
//! `/api/knowledge` would. With `--semantic`, the semantic providers then run once and the graph
//! is rebuilt with their facts. `--slowest N` lowers every file again, one at a time, and lists
//! the N slowest. The `nodex_perf` lines on stderr break each step down further.
//!
//! Opens go through the lowering cache the server uses; the folder's cache is removed first, so
//! run 1 is a cold open and later runs are warm reopens. `--no-cache` opens without it.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use nodex_core::Project;

struct Args {
    folder: PathBuf,
    semantic: bool,
    runs: usize,
    slowest: usize,
    cache: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut folder = None;
    let mut semantic = false;
    let mut runs = 1;
    let mut slowest = 0;
    let mut cache = true;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--semantic" => semantic = true,
            "--no-cache" => cache = false,
            "--runs" => runs = args.next().and_then(|n| n.parse().ok()).ok_or("--runs needs a number")?,
            "--slowest" => slowest = args.next().and_then(|n| n.parse().ok()).ok_or("--slowest needs a number")?,
            _ if folder.is_none() => folder = Some(PathBuf::from(arg)),
            _ => return Err(format!("unexpected argument {arg}")),
        }
    }
    let folder = folder.ok_or("usage: bench_open <folder> [--semantic] [--runs N] [--slowest N] [--no-cache]")?;
    Ok(Args { folder, semantic, runs: runs.max(1), slowest, cache })
}

fn ms(d: Duration) -> String {
    format!("{:>9.1} ms", d.as_secs_f64() * 1000.0)
}

fn mb(bytes: usize) -> String {
    format!("{:.2} MB", bytes as f64 / 1_048_576.0)
}

fn time<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let started = Instant::now();
    let value = f();
    (value, started.elapsed())
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "nodex_perf=info".into());
    tracing_subscriber::fmt().with_env_filter(filter).with_writer(std::io::stderr).with_target(false).init();

    println!("== detect, per language");
    for frontend in nodex_server::frontends() {
        let (found, t) = time(|| frontend.detect(&args.folder));
        println!("{:<22} {}   {}", format!("{:?}", frontend.id()), ms(t), if found.is_some() { "found" } else { "-" });
    }

    let cache = args.cache.then(nodex_server::projects::cache_dir);
    if let Some(base) = &cache
        && let Err(e) = nodex_core::cache::clear(base, &args.folder)
    {
        eprintln!("cannot clear the lowering cache: {e}");
    }
    let mut last = None;
    for run in 1..=args.runs {
        println!("\n== run {run}/{} : {}", args.runs, args.folder.display());
        let (project, open) =
            time(|| Project::open_cached(&args.folder, nodex_server::frontends(), cache.as_deref(), &|_| {}));
        let mut project = match project {
            Ok(p) => p,
            Err(e) => {
                eprintln!("cannot open: {e}");
                std::process::exit(1);
            }
        };
        let (summary, summary_t) = time(|| serde_json::to_vec(&project.summary()).unwrap());
        let (graph, structure_t) = time(|| project.structure());
        let (knowledge, knowledge_t) = time(|| project.knowledge());
        let (graph_json, graph_json_t) = time(|| nodex_server::builds::structure_wire(&graph).unwrap());
        let (knowledge_json, knowledge_json_t) = time(|| serde_json::to_vec(&*knowledge).unwrap());
        let files = project.files().count();
        let flows = project.flows().count();
        println!("open (walk + lower)    {}   {files} files, {flows} flows, {} from the cache", ms(open), project.files_from_cache());
        println!("summary json           {}   {}", ms(summary_t), mb(summary.len()));
        println!("structure + knowledge  {}   {} symbols, {} edges", ms(structure_t), graph.symbols.len(), graph.edges.len());
        println!("knowledge (cached)     {}", ms(knowledge_t));
        println!("structure json         {}   {}  hash {:016x}", ms(graph_json_t), mb(graph_json.len()), hash(&graph_json));
        println!("knowledge json         {}   {}  hash {:016x}", ms(knowledge_json_t), mb(knowledge_json.len()), hash(&knowledge_json));
        println!("time to first data     {}", ms(open + structure_t + graph_json_t));
        last = Some(project);
    }

    let Some(mut project) = last else { return };
    if args.slowest > 0 {
        slowest_files(&project, args.slowest);
    }
    if !args.semantic {
        return;
    }
    println!("\n== semantic");
    let root = project.root().to_path_buf();
    for frontend in project.active_frontends() {
        let language = frontend.id();
        let (provider, start_t) = time(|| frontend.semantic(&root));
        let mut provider = match provider {
            None => {
                println!("{language:?}: no semantic provider");
                continue;
            }
            Some(Err(reason)) => {
                println!("{language:?}: unavailable ({reason})");
                continue;
            }
            Some(Ok(p)) => p,
        };
        println!("{language:?} start            {}   {}", ms(start_t), provider.describe());
        let (facts, analyze_t) = time(|| provider.analyze());
        match facts {
            Ok(facts) => {
                println!("{language:?} analyze          {}   {} refs", ms(analyze_t), facts.refs.len());
                let ((), apply_t) = time(|| project.apply_semantic(language, facts));
                println!("{language:?} apply            {}", ms(apply_t));
            }
            Err(e) => println!("{language:?} analyze failed after {}: {e}", ms(analyze_t)),
        }
    }
    let (graph, rebuild_t) = time(|| project.structure());
    let (graph_json, graph_json_t) = time(|| nodex_server::builds::structure_wire(&graph).unwrap());
    println!("rebuild with facts     {}   {} symbols, {} edges", ms(rebuild_t), graph.symbols.len(), graph.edges.len());
    println!("structure json         {}   {}  hash {:016x}", ms(graph_json_t), mb(graph_json.len()), hash(&graph_json));
}

/// A hash of a payload, to check that a change leaves the output identical.
fn hash(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

/// Lowers every file again on this thread and prints the slowest ones.
fn slowest_files(project: &Project, count: usize) {
    let frontends = project.frontends();
    let mut timings: Vec<(Duration, String, usize)> = Vec::new();
    let mut total = Duration::ZERO;
    for (rel, _) in project.files() {
        let Some(path) = project.absolute(rel) else { continue };
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else { continue };
        let Some(frontend) = frontends.iter().find(|f| f.extensions().contains(&ext)) else { continue };
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let len = text.len();
        let source = nodex_lang_api::SourceFile { rel_path: rel.to_owned(), text };
        let ((), t) = time(|| drop(frontend.lower_file(&source)));
        total += t;
        timings.push((t, rel.to_owned(), len));
    }
    timings.sort_by_key(|t| std::cmp::Reverse(t.0));
    println!("
== slowest files (sequential lowering, total {})", ms(total));
    for (t, rel, len) in timings.iter().take(count) {
        println!("{}  {:>7} KB  {rel}", ms(*t), len / 1024);
    }
}
