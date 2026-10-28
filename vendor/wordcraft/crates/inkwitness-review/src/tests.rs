//! The writer → DOCX → review path through the real `WordApp`, without a browser.

use serde_json::{Value, json};
use wordcraft_doc::{Document, StoryRef};
use wordcraft_engine::Session;
use wordcraft_inkwitness::{Env, uuid_v4};
use wordcraft_ui_egui::{Services, WordApp};

use super::Route;
use super::review::{Consistency, Review, start_session};

struct TestEnv {
    ms: u64,
    ids: u64,
}

impl Env for TestEnv {
    fn now_iso(&mut self) -> String {
        "2026-10-28T09:00:00Z".into()
    }
    fn monotonic_ms(&mut self) -> u64 {
        self.ms += 100;
        self.ms
    }
    fn uuid(&mut self) -> String {
        self.ids += 1;
        let mut b = [0u8; 16];
        b[8..].copy_from_slice(&self.ids.to_be_bytes());
        uuid_v4(b)
    }
}

fn text_185() -> String {
    let s: String = "Example Student pasted this synthetic passage to test provenance. ".repeat(3).chars().take(185).collect();
    assert_eq!(s.len(), 185);
    s
}

fn run(app: &mut WordApp, id: &str, params: Value) {
    app.run(id, params).unwrap_or_else(|e| panic!("{id}: {e}"));
}

/// Write → paste 185 characters → delete → undo → Save, as the writer page does; the DOCX bytes.
fn written_docx() -> Vec<u8> {
    let mut app = WordApp::new(Session::new(Document::new()), Services::default());
    super::writer::attach(&mut app, Box::new(TestEnv { ms: 0, ids: 0 }), Box::new(|| "2026-10-28T09:05:00Z".to_string()));
    for c in "Example Student wrote this. ".chars() {
        run(&mut app, "text.insert", json!({"text": c.to_string()}));
    }
    run(&mut app, "edit.paste", json!({"text": text_185()}));
    run(&mut app, "text.backspace", json!({}));
    run(&mut app, "edit.undo", json!({}));
    let prepare = app.services.prepare_save.as_ref().unwrap_or_else(|| panic!("no save hook"));
    let doc = prepare("essay.docx", &app.session.doc).unwrap_or_else(|e| panic!("{e}")).unwrap_or_else(|| panic!("nothing embedded"));
    assert!(prepare("essay.pdf", &app.session.doc).unwrap_or_else(|e| panic!("{e}")).is_none(), "only a Word document carries the history");
    wordcraft_engine::io::save_bytes("essay.docx", &doc).unwrap_or_else(|e| panic!("{e}"))
}

fn review_app() -> WordApp {
    WordApp::new(start_session(Document::new()), Services::default())
}

#[test]
fn routes_come_from_the_url() {
    assert_eq!(Route::from_url("/InkWitness/review/", ""), Route::Review);
    assert_eq!(Route::from_url("/InkWitness/review/index.html", ""), Route::Write);
    assert_eq!(Route::from_url("/review.html", ""), Route::Review);
    assert_eq!(Route::from_url("/InkWitness/write/", ""), Route::Write);
    assert_eq!(Route::from_url("/", "?mode=review"), Route::Review);
    assert_eq!(Route::from_url("/", "?sample&mode=review"), Route::Review);
    // Parsed, not searched: these name review without asking for it.
    assert_eq!(Route::from_url("/", "?mode=reviewer"), Route::Write);
    assert_eq!(Route::from_url("/", "?title=mode=review"), Route::Write);
    assert_eq!(Route::from_url("/reviews/", ""), Route::Write);
}

#[test]
fn review_seeks_to_the_paste_and_highlights_exactly_the_pasted_text() {
    let bytes = written_docx();
    let mut app = review_app();
    let mut review = Review::default();
    review.open(&mut app, "essay.docx", &bytes);
    let l = review.loaded.as_ref().unwrap_or_else(|| panic!("not opened: {:?}", review.error));
    assert_eq!(l.consistency, Consistency::Consistent);
    assert_eq!(l.replayable, l.replay.last_seq());
    // Opened at the last event: the undo put back the deleted character.
    assert_eq!(app.session.doc.plain_text(StoryRef::Body), format!("Example Student wrote this. {}", text_185()));
    let paste = review.first_paste().unwrap_or_else(|| panic!("no paste"));
    review.seek(&mut app, paste);
    assert_eq!(review.highlighted_text().as_deref(), Some(text_185().as_str()));
    let (a, b) = app.canvas.review_mark.clone().unwrap_or_else(|| panic!("nothing highlighted"));
    assert_eq!((a.off, b.off), (28, 213));
    assert_eq!(app.session.sel.focus, a, "the caret is at the paste, so the view scrolls to it");
    assert!(app.canvas.scroll_to_caret);
    let note = review.note().join("\n");
    assert!(note.contains("Type: paste") && note.contains("Inserted: 185 characters") && note.contains("gestureId: "), "{note}");
    assert!(!note.contains("gestureId: none"), "{note}");
    assert!(review.status_line().contains("Local history consistent"));
    // Stepping back and forth rebuilds the states in between.
    review.seek(&mut app, paste - 1);
    assert_eq!(app.session.doc.plain_text(StoryRef::Body), "Example Student wrote this. ");
    review.seek(&mut app, paste + 1);
    assert!(app.session.doc.plain_text(StoryRef::Body).ends_with(&text_185()[..184]));
    review.seek(&mut app, 0);
    assert_eq!(app.session.doc.plain_text(StoryRef::Body), "");
}

#[test]
fn the_reviewed_document_cannot_be_changed() {
    let bytes = written_docx();
    let mut app = review_app();
    let mut review = Review::default();
    review.open(&mut app, "essay.docx", &bytes);
    let paste = review.first_paste().unwrap_or(0);
    review.seek(&mut app, paste);
    let shown = app.session.doc.clone();
    for (id, p) in [
        ("text.insert", json!({"text": "x"})),
        ("edit.paste", json!({"text": "y"})),
        ("text.backspace", json!({})),
        ("edit.undo", json!({})),
        ("layout.margins", json!({"preset": "narrow"})),
        ("file.new", json!({})),
        ("file.save", json!({"path": "copy.docx"})),
    ] {
        assert!(app.run(id, p).is_err(), "{id} ran in review");
        assert!(app.session.doc.same_content(&shown), "{id} changed the reviewed document");
    }
    // Looking around is fine.
    assert!(app.run("caret.docEnd", json!({})).is_ok());
    assert!(app.run("select.all", json!({})).is_ok());
    // A change made around the session is put back on the next frame.
    app.session.doc = Document::new();
    review.logic(&mut app);
    assert!(app.session.doc.same_content(&shown));
}

#[test]
fn a_file_without_history_or_with_a_broken_one_says_so() {
    let mut app = review_app();
    let mut review = Review::default();
    let plain = wordcraft_engine::io::save_bytes("plain.docx", &Document::new()).unwrap_or_default();
    review.open(&mut app, "plain.docx", &plain);
    assert!(review.loaded.is_none());
    assert!(review.error.as_deref().is_some_and(|e| e.contains("History missing")), "{:?}", review.error);
    review.open(&mut app, "junk.docx", b"not a zip");
    assert!(review.loaded.is_none() && review.error.is_some());
    // Text changed outside InkWitness after the last event.
    let bytes = written_docx();
    let mut doc = wordcraft_engine::io::open_bytes("essay.docx", &bytes).unwrap_or_default();
    let mut s = Session::new(doc.clone());
    let _ = s.run("caret.docEnd", &json!({}));
    let _ = s.run("text.insert", &json!({"text": " Added in another editor."}));
    doc = s.doc.clone();
    let edited = wordcraft_engine::io::save_bytes("essay.docx", &doc).unwrap_or_default();
    review.open(&mut app, "edited.docx", &edited);
    assert_eq!(review.loaded.as_ref().map(|l| l.consistency.clone()), Some(Consistency::ExternallyChanged));
}

#[test]
fn the_panel_draws_without_a_file_and_with_one() {
    let bytes = written_docx();
    let mut app = review_app();
    let mut review = Review::default();
    let ctx = egui::Context::default();
    let frame = |review: &mut Review, app: &mut WordApp| {
        ctx.run_ui(egui::RawInput::default(), |ui| review.panel(ui, app)).drop_without_applying_deltas();
    };
    frame(&mut review, &mut app);
    review.open(&mut app, "essay.docx", &bytes);
    frame(&mut review, &mut app);
    assert!(review.status_changed().is_some());
    assert!(review.status_changed().is_none(), "published once");
}
