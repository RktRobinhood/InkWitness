//! The synthetic evidence fixture (`tests/fixtures/g0-evidence.docx`): a short session typed and
//! pasted into a real WordCraft `Session`, its events recorded by the real `Recorder`, wrapped in
//! the envelope and saved by the real WordCraft DOCX writer. Everything is fixed (clock, ids,
//! datastore item id), so it builds to the same bytes every time.
//!
//! Regenerate: `cargo run -p wordcraft-inkwitness --example evidence_fixture` (from
//! `vendor/wordcraft/`).

use serde_json::{Value, json};
use wordcraft_doc::Document;
use wordcraft_doc::Pos;
use wordcraft_engine::Selection;
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

/// The long fixture's path, from the crate (`g2-pages.docx`, FAL-478): an essay of seven pages,
/// edits on page 3 and a 185-character paste on page 7, for the review page's scrolling.
pub const PAGES_PATH: &str = "tests/fixtures/g2-pages.docx";
/// How many paragraphs the long fixture's essay has before the paste.
pub const PAGES_PARAGRAPHS: usize = 28;
/// Words in each of them.
pub const PAGES_WORDS: usize = 90;
/// The 185-character passage pasted on the last page.
pub const PAGES_PASTE: &str = "Example Student pasted this synthetic passage at the end of the essay so the review page has a flagged moment on page seven. It is fabricated test text and is not taken from any source.";

fn words(paragraph: usize) -> String {
    const VOCAB: [&str; 24] = [
        "evidence",
        "knowledge",
        "claim",
        "perspective",
        "method",
        "question",
        "reasoning",
        "context",
        "source",
        "belief",
        "justify",
        "pattern",
        "community",
        "culture",
        "measure",
        "doubt",
        "theory",
        "observe",
        "explain",
        "limit",
        "assume",
        "compare",
        "record",
        "reflect",
    ];
    let mut out: Vec<&str> = Vec::with_capacity(PAGES_WORDS);
    for i in 0..PAGES_WORDS {
        out.push(VOCAB[(paragraph * 7 + i * 5 + i / 11) % VOCAB.len()]);
    }
    let mut text = out.join(" ");
    text.push('.');
    text
}

/// The page (0-based) the paragraph `block` starts on.
pub fn page_of(doc: &Document, block: usize) -> Option<usize> {
    let lay = wordcraft_layout::layout(doc, &mut wordcraft_layout::LayoutCache::new(), &Default::default());
    lay.caret_on(&Pos::body(block, 0), 0).map(|c| c.page)
}

/// The long session: type [`PAGES_PARAGRAPHS`] paragraphs, edit one on page 3 (type, delete,
/// type), then paste [`PAGES_PASTE`] at the end, on page 7.
pub fn pages_session() -> (Session, Recorder) {
    let mut s = Session::new(Document::new());
    let config = Config { attempt_id: ATTEMPT.into(), session_span_id: "span-1".into(), field_id: "essay".into(), section_id: Some("main".into()) };
    let r = Recorder::attach(&mut s, Box::new(FixedEnv { ms: 0, ids: 0 }), config);
    for p in 0..PAGES_PARAGRAPHS {
        if p > 0 {
            run(&mut s, "text.newParagraph", json!({}));
        }
        for c in words(p).chars() {
            run(&mut s, "text.insert", json!({"text": c.to_string()}));
        }
    }
    // Back on page 3: the first paragraph that starts there.
    let third = (0..PAGES_PARAGRAPHS).find(|b| page_of(&s.doc, *b) == Some(2)).unwrap_or(0);
    s.sel = Selection::caret(Pos::body(third, 40));
    for c in " (revised)".chars() {
        run(&mut s, "text.insert", json!({"text": c.to_string()}));
    }
    for _ in 0..4 {
        run(&mut s, "text.backspace", json!({}));
    }
    for c in "noted".chars() {
        run(&mut s, "text.insert", json!({"text": c.to_string()}));
    }
    // And the paste at the very end.
    let last = PAGES_PARAGRAPHS - 1;
    let end = s.doc.plain_text(wordcraft_doc::StoryRef::Body).split('\n').next_back().map_or(0, |l| l.chars().count());
    s.sel = Selection::caret(Pos::body(last, end));
    s.begin_gesture(Gesture::paste("edit.paste", Some(vec!["text/plain".into()])));
    run(&mut s, "edit.paste", json!({"text": PAGES_PASTE}));
    s.end_gesture();
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
    embed(&s, &r)
}

/// The long fixture's DOCX bytes.
pub fn build_pages() -> Vec<u8> {
    let (s, r) = pages_session();
    embed(&s, &r)
}

fn embed(s: &Session, r: &Recorder) -> Vec<u8> {
    let payload = payload(s, r);
    let mut doc = s.doc.clone();
    if let Err(e) = envelope::embed_with_item_id(&mut doc, ATTEMPT, &payload, ITEM_ID) {
        panic!("embed: {e}");
    }
    match wordcraft_docx::write(&doc) {
        Ok(b) => b,
        Err(e) => panic!("write: {e}"),
    }
}
