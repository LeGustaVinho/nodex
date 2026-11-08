//! Prints what a `rust-analyzer scip` index contains (a diagnostic aid for `semantic.rs`).
//!
//! Usage: cargo run -p nodex-lang-rust --example scip_dump -- <index.scip> [file-filter]

use protobuf::Message;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bytes = std::fs::read(&args[0]).expect("index file");
    let index = scip::types::Index::parse_from_bytes(&bytes).expect("valid SCIP");
    let filter = args.get(1).cloned().unwrap_or_default();
    println!("tool: {:?}  project: {}", index.metadata.tool_info.name, index.metadata.project_root);
    println!("documents: {}  external symbols: {}", index.documents.len(), index.external_symbols.len());
    for doc in &index.documents {
        if !doc.relative_path.contains(&filter) {
            continue;
        }
        println!("== {} ({:?}) occurrences={} symbols={}", doc.relative_path, doc.position_encoding, doc.occurrences.len(), doc.symbols.len());
        for s in doc.symbols.iter().take(40) {
            let sig = s.signature_documentation.as_ref().map(|d| d.text.clone()).unwrap_or_default();
            let rel: Vec<String> = s.relationships.iter().map(|r| format!("{}{}", if r.is_implementation { "impl " } else { "" }, r.symbol)).collect();
            println!("  sym {:?} {} | {} | {:?}", s.kind.enum_value_or_default(), s.symbol, sig.replace('\n', " "), rel);
        }
        for o in doc.occurrences.iter().take(60) {
            println!("  occ {:?} roles={} {}", o.range, o.symbol_roles, o.symbol);
        }
    }
    for s in index.external_symbols.iter().take(10) {
        let sig = s.signature_documentation.as_ref().map(|d| d.text.clone()).unwrap_or_default();
        println!("  ext {:?} {} | {}", s.kind.enum_value_or_default(), s.symbol, sig.replace('\n', " "));
    }
}
