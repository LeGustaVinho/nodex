// Rebuild when the web UI is (re)built, so the embedded assets stay current.
fn main() {
    println!("cargo:rerun-if-changed=../../web/dist");
}
