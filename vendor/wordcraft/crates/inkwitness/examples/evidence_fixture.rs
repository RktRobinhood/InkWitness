//! Write the synthetic evidence fixture DOCX (see `tests/common/fixture.rs`).
//!
//! `cargo run -p wordcraft-inkwitness --example evidence_fixture [-- <out.docx>]` (from
//! `vendor/wordcraft/`); the default output is `crates/inkwitness/tests/fixtures/g0-evidence.docx`.

#[path = "../tests/common/fixture.rs"]
mod fixture;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| format!("{}/{}", env!("CARGO_MANIFEST_DIR"), fixture::PATH));
    let bytes = fixture::build();
    if let Err(e) = std::fs::write(&out, &bytes) {
        eprintln!("{out}: {e}");
        std::process::exit(1);
    }
    println!("wrote {out} ({} bytes)", bytes.len());
}
