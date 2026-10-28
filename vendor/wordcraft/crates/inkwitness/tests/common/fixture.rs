//! The synthetic evidence fixture (`tests/fixtures/g0-evidence.docx`): a short session typed and
//! pasted into a real WordCraft `Session`, its events recorded by the real `Recorder`, wrapped in
//! the envelope and saved by the real WordCraft DOCX writer. Everything is fixed (clock, ids,
//! datastore item id), so it builds to the same bytes every time.
//!
//! Regenerate: `cargo run -p wordcraft-inkwitness --example evidence_fixture` (from
//! `vendor/wordcraft/`).

use serde_json::{Value, json};
use wordcraft_doc::Document;
use wordcraft_engine::Session;
use wordcraft_engine::observe::Gesture;
use wordcraft_inkwitness::{Config, Env, Recorder, envelope, uuid_v4};

/// The attempt id of the `docs/DOCX_FORMAT.md` example.
pub const ATTEMPT: &str = "00000000-0000-4000-8000-000000000001";
/// The datastore item id of the fixture's envelope part.
pub const ITEM_ID: &str = "{1A2B3C4D-0000-4000-8000-000000000001}";
pub const TYPED: &str = "Example Student wrote this.";
pub const PASTED: &str = " A synthetic pasted sentence.";
/// The fixture's path, from the crate.
pub const PATH: &str = "tests/fixtures/g0-evidence.docx";

/// A fixed clock and counting ids.
struct FixedEnv {
    ms: u64,
    ids: u64,
}

impl Env for FixedEnv {
    fn now_iso(&mut self) -> String {
        "2026-10-28T09:00:00Z".into()
    }
    fn monotonic_ms(&mut self) -> u64 {
        self.ms += 250;
        self.ms
    }
    fn uuid(&mut self) -> String {
        self.ids += 1;
        let mut b = [0u8; 16];
        b[8..].copy_from_slice(&self.ids.to_be_bytes());
        uuid_v4(b)
    }
}

fn run(s: &mut Session, id: &str, params: Value) {
    if let Err(e) = s.run(id, &params) {
        panic!("{id}: {e}");
    }
}

/// The session: type [`TYPED`] key by key, paste [`PASTED`], delete one character, undo it.
pub fn session() -> (Session, Recorder) {
    let mut s = Session::new(Document::new());
    let config = Config { attempt_id: ATTEMPT.into(), session_span_id: "span-1".into(), field_id: "essay".into(), section_id: Some("main".into()) };
    let r = Recorder::attach(&mut s, Box::new(FixedEnv { ms: 0, ids: 0 }), config);
    for c in TYPED.chars() {
        run(&mut s, "text.insert", json!({"text": c.to_string()}));
    }
    s.begin_gesture(Gesture::paste("edit.paste", Some(vec!["text/plain".into()])));
    run(&mut s, "edit.paste", json!({"text": PASTED}));
    s.end_gesture();
    run(&mut s, "text.backspace", json!({}));
    run(&mut s, "edit.undo", json!({}));
    (s, r)
}

/// The evidence JSON (`schemas/evidence-v1.schema.json`) for the session.
pub fn payload(s: &Session, r: &Recorder) -> Vec<u8> {
    let events = r.events();
    let doc = json!({
        "schema": "urn:workalone:evidence",
        "schemaVersion": "1.0",
        "writerVersion": "inkwitness-g0-fixture",
        "assignmentId": "g0-synthetic-fixture",
        "attemptId": ATTEMPT,
        "student": {
            "displayName": "Example Student",
            "studentCode": null,
            "enteredAtLocal": "2026-10-28T09:00:00Z",
            "identityAuthenticated": false,
            "identityEnteredByStudent": true
        },
        "policy": {
            "aiAssistance": "not-permitted",
            "clipboard": "allow-record",
            "policyVersion": "synthetic-fixture",
            "acknowledgedAtLocal": "2026-10-28T09:00:00Z"
        },
        "sections": [{"sectionId": "main", "fields": [{"fieldId": "essay", "kind": "richtext"}]}],
        "sessionSpans": [{"spanId": "span-1", "startedAtLocal": "2026-10-28T09:00:00Z", "startedElapsedMs": 0, "attemptId": ATTEMPT, "reason": "new"}],
        "events": events,
        "checkpoints": r.checkpoints(),
        "diagnostics": [],
        "export": {
            "finalSeq": events.len(),
            "exportedAtLocal": "2026-10-28T09:05:00Z",
            "recordedStateDigest": r.state_digest(&s.doc),
            "integrityType": "unsigned-local-chain",
            "selfHashCaveat": "Unsigned local evidence; not proof of authenticity"
        }
    });
    serde_json::to_vec(&doc).unwrap_or_default()
}

/// The fixture DOCX bytes.
pub fn build() -> Vec<u8> {
    let (s, r) = session();
    let payload = payload(&s, &r);
    let mut doc = s.doc.clone();
    if let Err(e) = envelope::embed_with_item_id(&mut doc, ATTEMPT, &payload, ITEM_ID) {
        panic!("embed: {e}");
    }
    match wordcraft_docx::write(&doc) {
        Ok(b) => b,
        Err(e) => panic!("write: {e}"),
    }
}
