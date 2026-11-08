//! Prints the flows of one Rust file with pins, wires and badges (a diagnostic aid).
//!
//! Usage: cargo run -p nodex-lang-rust --example rs_dump -- <file.rs> [function]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let text = std::fs::read_to_string(&args[0]).expect("file");
    let ir = nodex_lang_rust::lower_source("src/x.rs", &text);
    for d in &ir.diagnostics {
        println!("diagnostic: {}", d.message);
    }
    for flow in ir.flows.iter().filter(|f| args.get(1).is_none_or(|n| f.symbol.ends_with(n.as_str()))) {
        println!("== {}", flow.symbol);
        for n in &flow.nodes {
            let outs: Vec<String> = n.outputs.iter().filter(|p| p.id.starts_with("var:")).map(|p| format!("{}:{}", p.label, p.ty.clone().unwrap_or_default())).collect();
            println!("  {:?} {} {:?} out={:?}", n.concept, n.label, n.flavors, outs);
            for e in flow.edges.iter().filter(|e| e.to == n.id && e.kind == nodex_ir::FlowEdgeKind::Data) {
                println!("      <- {} {:?}", e.to_pin, e.access);
            }
        }
    }
}
