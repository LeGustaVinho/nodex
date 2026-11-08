//! The whole structure graph and knowledge of the fixture projects, as snapshots: one line per
//! symbol and per edge, in graph order. Performance work on linking and knowledge (indexes in
//! place of scans, builds off the lock) must not change a single line.

use std::path::PathBuf;
use std::sync::Arc;

use nodex_core::Project;
use nodex_lang_api::LanguageFrontend;
use nodex_lang_csharp::CSharpFrontend;
use nodex_lang_rust::RustFrontend;
use nodex_lang_typescript::TypeScriptFrontend;

fn digest(rel: &str) -> String {
    let frontends: Vec<Arc<dyn LanguageFrontend>> =
        vec![Arc::new(CSharpFrontend), Arc::new(TypeScriptFrontend), Arc::new(RustFrontend)];
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(rel);
    let mut project = Project::open(&root, frontends).expect("fixture opens");
    let graph = project.structure();
    let knowledge = project.knowledge();
    let mut out = format!("{} symbols, {} edges\n\n# symbols\n", graph.symbols.len(), graph.edges.len());
    for s in &graph.symbols {
        out.push_str(&serde_json::to_string(s).unwrap());
        out.push('\n');
    }
    out.push_str("\n# edges\n");
    for e in &graph.edges {
        out.push_str(&serde_json::to_string(e).unwrap());
        out.push('\n');
    }
    out.push_str("\n# knowledge\n");
    out.push_str(&serde_json::to_string_pretty(&*knowledge).unwrap());
    out
}

#[test]
fn csharp_sample() {
    insta::assert_snapshot!(digest("csharp/Sample"));
}

#[test]
fn typescript_sample() {
    insta::assert_snapshot!(digest("ts/Sample"));
}

#[test]
fn rust_sample() {
    insta::assert_snapshot!(digest("rust/Sample"));
}

#[test]
fn fullstack() {
    insta::assert_snapshot!(digest("knowledge/fullstack"));
}

#[test]
fn unity_sample() {
    insta::assert_snapshot!(digest("unity-sample"));
}
