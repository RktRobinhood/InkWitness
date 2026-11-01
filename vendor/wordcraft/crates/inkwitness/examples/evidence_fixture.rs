//! Write the synthetic evidence fixture DOCX (see `tests/common/fixture.rs`).
//!
//! `cargo run -p wordcraft-inkwitness --example evidence_fixture [-- <out.docx>]` (from
//! `vendor/wordcraft/`); the default output is `crates/inkwitness/tests/fixtures/g0-evidence.docx`.

#[path = "../tests/common/fixture.rs"]
mod fixture;

fn main() {
    // `-- pages [out.docx]` writes the long fixture (g2-pages.docx) instead.
    let mut args = std::env::args().skip(1).peekable();
    let pages = args.peek().is_some_and(|a| a == "pages");
    if pages {
        args.next();
    }
    let default = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), if pages { fixture::PAGES_PATH } else { fixture::PATH });
    let out = args.next().unwrap_or(default);
    let bytes = if pages { fixture::build_pages() } else { fixture::build() };
    if let Err(e) = std::fs::write(&out, &bytes) {
        eprintln!("{out}: {e}");
        std::process::exit(1);
    }
    println!("wrote {out} ({} bytes)", bytes.len());
}
