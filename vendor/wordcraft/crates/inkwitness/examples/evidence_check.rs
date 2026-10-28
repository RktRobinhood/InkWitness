//! Report the evidence a DOCX carries, read with the real WordCraft reader:
//! `cargo run -p wordcraft-inkwitness --example evidence_check -- <file.docx>...`
//! (from `vendor/wordcraft/`). Exit status 0 only when every file has one readable envelope.

use wordcraft_inkwitness::envelope;

fn check(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let doc = wordcraft_docx::read(&bytes).map_err(|e| format!("not readable as DOCX: {e}"))?;
    let part = wordcraft_docx::find_custom_xml_part(&doc, envelope::NAMESPACE).map_err(|e| e.to_string())?;
    let Some(part) = part else { return Err("history missing: no custom XML part in the evidence namespace".into()) };
    let e = envelope::decode(&part.bytes).map_err(|e| format!("history corrupt ({}): {e}", part.part))?;
    let events = serde_json::from_slice::<serde_json::Value>(&e.payload).ok().and_then(|v| v["events"].as_array().map(Vec::len));
    let digest = wordcraft_inkwitness::sha256_hex("inkwitness:payload-check", &e.payload);
    Ok(format!(
        "evidence found in {} (item id {}): attempt {}, {} payload bytes, {} events, payload check {}",
        part.part,
        part.item_id.as_deref().unwrap_or("none"),
        e.attempt_id,
        e.payload.len(),
        events.map_or("?".into(), |n| n.to_string()),
        &digest[..16]
    ))
}

fn main() {
    let mut ok = true;
    for path in std::env::args().skip(1) {
        match check(&path) {
            Ok(s) => println!("{path}: {s}"),
            Err(e) => {
                ok = false;
                println!("{path}: {e}");
            }
        }
    }
    std::process::exit(if ok { 0 } else { 1 });
}
