//! Prints what the TypeScript frontend produces for files: diagnostics, grammar failures,
//! symbols and every flow node (concept, label, flavors), flagging `Opaque` code.
//! Usage: cargo run -p nodex-lang-typescript --example ts_coverage -- <file>... [--quiet]

use nodex_ir::{Concept, Expr};

fn opaque_exprs<'a>(e: &'a Expr, out: &mut Vec<&'a Expr>) {
    if e.concept == Concept::Opaque {
        out.push(e);
    }
    for c in &e.children {
        opaque_exprs(c, out);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let quiet = args.iter().any(|a| a == "--quiet");
    for path in args.iter().filter(|a| !a.starts_with("--")) {
        let text = std::fs::read_to_string(path).expect("readable file");
        let rel = path.replace('\\', "/");
        let name = rel.rsplit('/').next().unwrap_or(&rel).to_owned();
        let ir = nodex_lang_typescript::lower_source(&name, &text);
        println!("=== {name}");
        for d in &ir.diagnostics {
            println!("  DIAG {:?} {} {:?}", d.severity, d.message, d.span.as_ref().map(|s| (s.start_line, s.start_col)));
        }
        for rule in nodex_lang_typescript::grammar_bridges(&name, &text) {
            println!("  BRIDGED {rule}");
        }
        for (line, snippet) in nodex_lang_typescript::remaining_parse_errors(&name, &text) {
            println!("  GRAMMAR @{line}: {snippet}");
        }
        if !quiet {
            for i in &ir.imports {
                println!("  IMPORT {} -> {}", i.from_scope, i.target);
            }
            for s in &ir.symbols {
                println!("  SYM {:?} {} [{}]{}", s.kind, s.id, s.flavors.join(","), if s.has_flow { " *flow" } else { "" });
            }
            for r in &ir.type_refs {
                println!("  REF {:?} {} -> {}", r.relation, r.from, r.name);
            }
        }
        for f in &ir.flows {
            if !quiet {
                println!("  FLOW {}", f.symbol);
            }
            for n in &f.nodes {
                let mut bad = Vec::new();
                if let Some(e) = &n.expr {
                    opaque_exprs(e, &mut bad);
                }
                if !quiet || n.concept == Concept::Opaque || !bad.is_empty() {
                    println!("    {:?} | {} | [{}]", n.concept, n.label, n.flavors.join(","));
                }
                for e in bad {
                    println!("      OPAQUE EXPR {} [{}]", e.label, e.flavors.join(","));
                }
            }
        }
    }
}
