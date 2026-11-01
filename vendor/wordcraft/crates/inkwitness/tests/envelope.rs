//! The evidence envelope in a real WordCraft DOCX (backlog G0-5; `docs/DOCX_FORMAT.md`).

#[path = "common/fixture.rs"]
mod fixture;

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use wordcraft_doc::{Document, Path, StoryRef};
use wordcraft_engine::Session;
use wordcraft_inkwitness::envelope::{self, EnvelopeError as E, MAX_ENVELOPE_BYTES, MAX_PAYLOAD_BYTES, NAMESPACE};

const ATTEMPT: &str = "6f1c2a3b-4d5e-4f60-8a7b-9c0d1e2f3a4b";

fn payload() -> Vec<u8> {
    serde_json::to_vec(&json!({"schema": "urn:workalone:evidence", "attemptId": ATTEMPT, "events": [{"seq": 1, "insertedText": "Example Student"}]}))
        .unwrap()
}

fn doc_with_text() -> Document {
    let mut doc = Document::new();
    doc.para_mut(StoryRef::Body, &Path::top(0)).unwrap().insert_text(0, "Example Student essay.", &Default::default()).unwrap();
    doc
}

fn unzip(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut b = Vec::new();
            f.read_to_end(&mut b).unwrap();
            (f.name().to_string(), b)
        })
        .collect()
}

fn zip(parts: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        z.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(bytes).unwrap();
    }
    z.finish().unwrap().into_inner()
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(data).unwrap();
    gz.finish().unwrap()
}

/// An envelope with `attrs` replacing the standard attributes when given, and `body` as text.
fn xml(attrs: Option<&str>, body: &str) -> Vec<u8> {
    let std_attrs =
        format!(r#"schemaVersion="1.0" payloadMediaType="application/vnd.workalone.history+json" encoding="gzip+base64" attemptId="{ATTEMPT}""#);
    format!(r#"<?xml version="1.0" encoding="utf-8"?><wa:history xmlns:wa="{NAMESPACE}" {}>{body}</wa:history>"#, attrs.unwrap_or(&std_attrs))
        .into_bytes()
}

fn b64_gz(data: &[u8]) -> String {
    STANDARD.encode(gzip(data))
}

#[test]
fn encode_then_decode_gives_the_same_payload_bytes_and_the_documented_shape() {
    let p = payload();
    let x = envelope::encode(ATTEMPT, &p).unwrap();
    assert_eq!(envelope::encode(ATTEMPT, &p).unwrap(), x, "deterministic");
    let s = String::from_utf8(x.clone()).unwrap();
    assert!(s.starts_with(r#"<?xml version="1.0" encoding="utf-8"?>"#));
    for a in [
        r#"<wa:history xmlns:wa="urn:workalone:evidence:v1""#,
        r#"schemaVersion="1.0""#,
        r#"payloadMediaType="application/vnd.workalone.history+json""#,
        r#"encoding="gzip+base64""#,
        &format!(r#"attemptId="{ATTEMPT}""#),
    ] {
        assert!(s.contains(a), "{a}");
    }
    let e = envelope::decode(&x).unwrap();
    assert_eq!((e.attempt_id.as_str(), e.schema_version.as_str(), e.payload), (ATTEMPT, "1.0", p.clone()));
    // Whitespace in the base64 text (a re-serializing editor may wrap it) is fine.
    let text = b64_gz(&p);
    let wrapped: String = text.as_bytes().chunks(76).map(|c| format!("\n  {}", std::str::from_utf8(c).unwrap())).collect();
    assert_eq!(envelope::decode(&xml(None, &format!("{wrapped}\n"))).unwrap().payload, p);
    // So is a re-save as UTF-16, either byte order, or with a UTF-8 byte order mark.
    let s = s.replace("utf-8", "UTF-16");
    let le: Vec<u8> = [0xFF, 0xFE].into_iter().chain(s.encode_utf16().flat_map(u16::to_le_bytes)).collect();
    let be: Vec<u8> = [0xFE, 0xFF].into_iter().chain(s.encode_utf16().flat_map(u16::to_be_bytes)).collect();
    let bom: Vec<u8> = [0xEF, 0xBB, 0xBF].into_iter().chain(x.iter().copied()).collect();
    for v in [le, be, bom] {
        assert_eq!(envelope::decode(&v).unwrap().payload, p);
    }
}

#[test]
fn write_then_read_a_docx_gives_the_same_payload_bytes() {
    let mut doc = doc_with_text();
    let p = payload();
    let part = envelope::embed(&mut doc, ATTEMPT, &p).unwrap();
    assert!(part.item_id.is_some_and(|id| id.len() == 38 && id.starts_with('{')));
    let bytes = wordcraft_docx::write(&doc).unwrap();
    let parts = unzip(&bytes);
    assert_eq!(parts.keys().filter(|k| k.starts_with("customXml/") && !k.contains("_rels")).count(), 2, "item and itemProps");
    let back = wordcraft_docx::read(&bytes).unwrap();
    assert_eq!(envelope::extract(&back).unwrap().unwrap().payload, p);
    assert_eq!(envelope::extract(&doc_with_text()), Ok(None), "no part: history missing");
}

#[test]
fn a_wordcraft_open_edit_save_cycle_keeps_the_evidence() {
    let mut bytes = fixture::build();
    let original = envelope::extract(&wordcraft_docx::read(&bytes).unwrap()).unwrap().unwrap();
    for round in 0..2 {
        // Open in a WordCraft session, edit with real commands, save.
        let mut s = Session::new(wordcraft_docx::read(&bytes).unwrap());
        for c in " More.".chars() {
            s.run("text.insert", &json!({"text": c.to_string()})).unwrap();
        }
        s.run("text.newParagraph", &json!({})).unwrap();
        s.run("text.insert", &json!({"text": "Example Student added a paragraph."})).unwrap();
        bytes = wordcraft_docx::write(&s.doc).unwrap();
        let back = wordcraft_docx::read(&bytes).unwrap();
        assert!(back.plain_text(StoryRef::Body).contains("added a paragraph"), "round {round}: the edit was saved");
        assert_eq!(envelope::extract(&back).unwrap().unwrap(), original, "round {round}: the evidence is unchanged");
        let p = unzip(&bytes);
        assert_eq!(p.keys().filter(|k| k.starts_with("customXml/") && !k.contains("_rels")).count(), 2, "round {round}: no copies");
    }
}

#[test]
fn the_envelope_coexists_with_another_namespace_and_the_bibliography() {
    let mut doc = doc_with_text();
    doc.sources.push(wordcraft_doc::Source { tag: "Example2026".into(), title: "Synthetic reference".into(), ..Default::default() });
    let other = br#"<o:data xmlns:o="urn:example:other">kept</o:data>"#;
    wordcraft_docx::set_custom_xml_part(&mut doc, "urn:example:other", other).unwrap();
    envelope::embed(&mut doc, ATTEMPT, &payload()).unwrap();
    let bytes = wordcraft_docx::write(&doc).unwrap();
    let parts = unzip(&bytes);
    assert!(String::from_utf8_lossy(&parts["customXml/item1.xml"]).contains("Sources"), "the bibliography keeps item1");
    let back = wordcraft_docx::read(&bytes).unwrap();
    assert_eq!(back.sources.len(), 1);
    assert_eq!(envelope::extract(&back).unwrap().unwrap().payload, payload());
    assert_eq!(wordcraft_docx::find_custom_xml_part(&back, "urn:example:other").unwrap().unwrap().bytes.as_slice(), other);
    // Embedding again replaces in place.
    let mut back = back;
    let newer = serde_json::to_vec(&json!({"attemptId": ATTEMPT, "events": []})).unwrap();
    envelope::embed(&mut back, ATTEMPT, &newer).unwrap();
    let again = wordcraft_docx::read(&wordcraft_docx::write(&back).unwrap()).unwrap();
    assert_eq!(envelope::extract(&again).unwrap().unwrap().payload, newer);
    assert_eq!(wordcraft_docx::custom_xml_parts(&again, NAMESPACE).len(), 1);
}

#[test]
fn hostile_envelopes_fail_cleanly() {
    let p = payload();
    let good = b64_gz(&p);
    let attrs = |v: &str, enc: &str, media: &str, attempt: &str| {
        format!(r#"schemaVersion="{v}" payloadMediaType="{media}" encoding="{enc}" attemptId="{attempt}""#)
    };
    let media = "application/vnd.workalone.history+json";
    let cases: Vec<(&str, Vec<u8>, fn(&E) -> bool)> = vec![
        ("oversize part", vec![b' '; MAX_ENVELOPE_BYTES + 1], |e| matches!(e, E::EnvelopeTooLarge(_))),
        ("gzip bomb", xml(None, &b64_gz(&vec![b' '; MAX_PAYLOAD_BYTES + 1])), |e| *e == E::PayloadTooLarge),
        ("bad base64 characters", xml(None, "!!!not base64!!!"), |e| *e == E::BadBase64),
        ("bad base64 length", xml(None, &good[..good.len() - 1]), |e| *e == E::BadBase64),
        ("empty payload", xml(None, ""), |e| *e == E::BadGzip),
        ("not gzip", xml(None, &STANDARD.encode(b"plain bytes, not a gzip stream")), |e| *e == E::BadGzip),
        ("truncated gzip", xml(None, &STANDARD.encode(&gzip(&p)[..20])), |e| *e == E::BadGzip),
        ("gzip with trailing bytes", xml(None, &STANDARD.encode([gzip(&p), b"junk".to_vec()].concat())), |e| *e == E::BadGzip),
        ("not JSON", xml(None, &b64_gz(b"not json")), |e| matches!(e, E::BadJson(_))),
        ("JSON array", xml(None, &b64_gz(b"[1,2]")), |e| matches!(e, E::BadJson(_))),
        ("attempt mismatch", xml(None, &b64_gz(br#"{"attemptId":"00000000-0000-4000-8000-000000000009"}"#)), |e| *e == E::AttemptMismatch),
        (
            "wrong namespace",
            format!(r#"<wa:history xmlns:wa="urn:other:evidence" {}>{good}</wa:history>"#, attrs("1.0", "gzip+base64", media, ATTEMPT)).into_bytes(),
            |e| *e == E::WrongNamespace,
        ),
        ("no namespace", format!(r#"<history {}>{good}</history>"#, attrs("1.0", "gzip+base64", media, ATTEMPT)).into_bytes(), |e| *e == E::WrongNamespace),
        ("wrong root", format!(r#"<wa:other xmlns:wa="{NAMESPACE}">{good}</wa:other>"#).into_bytes(), |e| *e == E::WrongNamespace),
        (
            "document type with an external entity",
            format!(
                r#"<?xml version="1.0"?><!DOCTYPE wa:history [<!ENTITY x SYSTEM "https://example.invalid/x">]><wa:history xmlns:wa="{NAMESPACE}" {}>&x;</wa:history>"#,
                attrs("1.0", "gzip+base64", media, ATTEMPT)
            )
            .into_bytes(),
            |e| *e == E::DocumentType,
        ),
        ("entity reference", xml(None, &format!("{good}&amp;")), |e| matches!(e, E::Malformed(_))),
        ("child element", xml(None, &format!("<wa:x/>{good}")), |e| matches!(e, E::Malformed(_))),
        ("CDATA", xml(None, &format!("<![CDATA[{good}]]>")), |e| matches!(e, E::Malformed(_))),
        ("unclosed", xml(None, &good).split_last().map(|(_, r)| r.to_vec()).unwrap(), |e| matches!(e, E::Malformed(_))),
        ("content after root", [xml(None, &good), b"<more/>".to_vec()].concat(), |e| matches!(e, E::Malformed(_))),
        ("empty root", format!(r#"<wa:history xmlns:wa="{NAMESPACE}" {}/>"#, attrs("1.0", "gzip+base64", media, ATTEMPT)).into_bytes(), |e| {
            matches!(e, E::Malformed(_))
        }),
        ("schema version 2", xml(Some(&attrs("2.0", "gzip+base64", media, ATTEMPT)), &good), |e| matches!(e, E::Unsupported(_))),
        ("plain encoding", xml(Some(&attrs("1.0", "base64", media, ATTEMPT)), &good), |e| matches!(e, E::Unsupported(_))),
        ("other media type", xml(Some(&attrs("1.0", "gzip+base64", "text/plain", ATTEMPT)), &good), |e| matches!(e, E::Unsupported(_))),
        ("bad attempt id", xml(Some(&attrs("1.0", "gzip+base64", media, "Example Student")), &good), |e| matches!(e, E::Malformed(_))),
        ("not XML", b"\x00\x01 not xml".to_vec(), |e| matches!(e, E::Malformed(_))),
        ("odd UTF-16", vec![0xFF, 0xFE, b'<'], |e| matches!(e, E::Malformed(_))),
        ("unpaired surrogate", vec![0xFF, 0xFE, 0x00, 0xD8, b'<', 0], |e| matches!(e, E::Malformed(_))),
    ];
    for (name, input, want) in cases {
        let got = envelope::decode(&input);
        assert!(matches!(&got, Err(e) if want(e)), "{name}: {got:?}");
    }
    assert!(envelope::decode(&xml(None, &good)).is_ok(), "the control case reads");
    // Writing refuses what reading would.
    assert!(matches!(envelope::encode("not-a-uuid", &p), Err(E::Malformed(_))));
    assert!(matches!(envelope::encode(ATTEMPT, b"not json"), Err(E::BadJson(_))));
    assert_eq!(envelope::encode("00000000-0000-4000-8000-000000000009", &p), Err(E::AttemptMismatch));
}

#[test]
fn every_single_byte_corruption_is_an_error_or_a_valid_read_never_a_panic() {
    let x = envelope::encode(ATTEMPT, &payload()).unwrap();
    for i in 0..x.len() {
        for b in [0u8, b'<', b'&', b'A', 0xff] {
            let mut y = x.clone();
            y[i] = b;
            if let Ok(e) = envelope::decode(&y) {
                assert_eq!(e.attempt_id.len(), 36);
            }
        }
    }
}

#[test]
fn a_docx_with_a_corrupt_duplicate_or_external_evidence_part_is_reported_not_guessed() {
    let bytes = fixture::build();
    let mut parts = unzip(&bytes);
    let (item, _) = parts
        .iter()
        .find(|(k, v)| k.starts_with("customXml/item") && v.starts_with(b"<?xml") && String::from_utf8_lossy(v).contains(NAMESPACE))
        .unwrap();
    let item = item.clone();

    // Corrupt payload: history corrupt, not missing.
    let mut bad = parts.clone();
    bad.insert(item.clone(), xml(None, "AAAA"));
    assert_eq!(envelope::extract(&wordcraft_docx::read(&zip(&bad)).unwrap()), Err(E::BadGzip));

    // Two evidence parts: ambiguous.
    let mut two = parts.clone();
    two.insert("customXml/copy.xml".into(), parts[&item].clone());
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone()).unwrap().replace(
        "</Relationships>",
        r#"<Relationship Id="rIdCopy" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml" Target="../customXml/copy.xml"/></Relationships>"#,
    );
    two.insert("word/_rels/document.xml.rels".into(), rels.into_bytes());
    assert_eq!(envelope::extract(&wordcraft_docx::read(&zip(&two)).unwrap()), Err(E::Ambiguous(2)));

    // Only an external relationship to the evidence: never fetched, so history missing.
    let rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone())
        .unwrap()
        .replace("Target=\"../customXml/item1.xml\"", "Target=\"https://example.invalid/customXml/item1.xml\" TargetMode=\"External\"");
    assert!(rels.contains("example.invalid"), "the fixture relates its evidence from the main document");
    parts.insert("word/_rels/document.xml.rels".into(), rels.into_bytes());
    parts.remove(&item);
    assert_eq!(envelope::extract(&wordcraft_docx::read(&zip(&parts)).unwrap()), Ok(None));
}

#[test]
fn the_committed_fixture_is_the_writers_output_and_reads_back() {
    let committed = std::fs::read(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), fixture::PATH)).expect("run the evidence_fixture example");
    assert!(committed == fixture::build(), "stale fixture: run `cargo run -p wordcraft-inkwitness --example evidence_fixture`");
    let doc = wordcraft_docx::read(&committed).unwrap();
    assert_eq!(doc.plain_text(StoryRef::Body), format!("{}{}", fixture::TYPED, fixture::PASTED));
    let e = envelope::extract(&doc).unwrap().unwrap();
    assert_eq!(e.attempt_id, fixture::ATTEMPT);
    let v: Value = serde_json::from_slice(&e.payload).unwrap();
    assert_eq!(v["student"]["displayName"], "Example Student");
    let events = v["events"].as_array().unwrap();
    let paste: Vec<&Value> = events.iter().filter(|e| e["type"] == "paste").collect();
    assert_eq!(paste.len(), 1);
    assert_eq!(paste[0]["insertedText"], fixture::PASTED);
    assert!(paste[0]["gestureId"].is_string());
    assert_eq!(v["export"]["finalSeq"], events.len());
    let part = wordcraft_docx::find_custom_xml_part(&doc, NAMESPACE).unwrap().unwrap();
    assert_eq!(part.item_id.as_deref(), Some(fixture::ITEM_ID));
}

#[test]
fn the_long_fixture_has_seven_pages_edits_on_page_three_and_a_paste_on_page_seven() {
    let (s, _) = fixture::pages_session();
    let lay = wordcraft_layout::layout(&s.doc, &mut wordcraft_layout::LayoutCache::new(), &Default::default());
    assert!(lay.pages.len() >= 7, "only {} pages", lay.pages.len());
    let committed =
        std::fs::read(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), fixture::PAGES_PATH)).expect("run the evidence_fixture example with `pages`");
    assert!(committed == fixture::build_pages(), "stale fixture: run `cargo run -p wordcraft-inkwitness --example evidence_fixture -- pages`");
    let e = envelope::extract(&wordcraft_docx::read(&committed).unwrap()).unwrap().unwrap();
    let v: Value = serde_json::from_slice(&e.payload).unwrap();
    let events = v["events"].as_array().unwrap();
    assert_eq!(events.iter().filter(|e| e["type"] == "paste").count(), 1);
    assert_eq!(events.iter().find(|e| e["type"] == "paste").unwrap()["insertedText"], fixture::PAGES_PASTE);
    assert_eq!(fixture::PAGES_PASTE.len(), 185);
    assert_eq!(v["student"]["displayName"], "Example Student");
}
