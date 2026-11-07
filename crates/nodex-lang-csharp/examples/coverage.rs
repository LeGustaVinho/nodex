//! Reports what the C# frontend does not map: grammar errors, `Opaque` statements and
//! expressions, and the symbols it produced.
//! Usage: cargo run -p nodex-lang-csharp --example coverage -- <file.cs>...

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
    for path in std::env::args().skip(1) {
        let text = std::fs::read_to_string(&path).expect("readable file");
        let name = std::path::Path::new(&path).file_name().unwrap().to_string_lossy().into_owned();
        let ir = nodex_lang_csharp::lower_source(&name, &text);
        println!("=== {name}");
        for d in &ir.diagnostics {
            println!("  DIAG {:?} {} {:?}", d.severity, d.message, d.span.as_ref().map(|s| (s.start_line, s.start_col)));
        }
        for (line, snippet) in nodex_lang_csharp::remaining_parse_errors(&text) {
            println!("  GRAMMAR @{line}: {snippet}");
        }
        for f in &ir.flows {
            for n in &f.nodes {
                if n.concept == Concept::Opaque {
                    println!("  OPAQUE-STMT {} [{}] {}", f.name, n.flavors.join(","), n.code);
                }
                if let Some(e) = &n.expr {
                    let mut out = Vec::new();
                    opaque_exprs(e, &mut out);
                    for o in out {
                        println!("  opaque-expr {} @{}: {}", f.name, o.span.start_line, o.label);
                    }
                }
            }
        }
        let mut kinds = std::collections::BTreeMap::<String, Vec<String>>::new();
        for s in &ir.symbols {
            kinds.entry(format!("{:?}", s.kind)).or_default().push(format!("{}{}", s.name, if s.has_flow { "*" } else { "" }));
        }
        for (k, names) in kinds {
            println!("  {k}: {}", names.join(", "));
        }
    }
}
