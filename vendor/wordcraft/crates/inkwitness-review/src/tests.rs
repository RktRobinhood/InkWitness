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
    // Shown as saved, with nothing to replay (FAL-448: the "History missing" state).
    let l = review.loaded.as_ref().unwrap_or_else(|| panic!("not opened: {:?}", review.error));
    assert_eq!((l.consistency.clone(), l.replayable), (Consistency::Missing, 0));
    assert!(review.status_line().contains("History missing"), "{}", review.status_line());
    review.open(&mut app, "junk.docx", b"not a zip");
    assert!(review.error.is_some());
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
fn the_chrome_draws_without_a_file_and_with_one() {
    let bytes = written_docx();
    let mut app = review_app();
    crate::toolbar::setup(&mut app);
    let mut review = Review::default();
    let ctx = egui::Context::default();
    // As the web shell runs a frame; the chrome waits, as the app does, for the fonts to be in use.
    let frame = |review: &mut Review, app: &mut WordApp| {
        ctx.run_ui(egui::RawInput::default(), |ui| {
            review.logic(app);
            app.logic(ui.ctx());
            review.chrome(ui, app);
            app.ui(ui);
        })
        .drop_without_applying_deltas();
    };
    for _ in 0..4 {
        frame(&mut review, &mut app);
    }
    review.open(&mut app, "essay.docx", &bytes);
    frame(&mut review, &mut app);
    assert!(review.status_changed().is_some());
    assert!(review.status_changed().is_none(), "published once");
}

/// The review page as the web shell draws it: [`crate::toolbar::setup`], the review's chrome, then
/// the app. `file` is opened first.
fn review_page(file: Option<(&str, &[u8])>) -> egui_kittest::Harness<'static, (Review, WordApp)> {
    let mut app = review_app();
    app.ui.language = "en".into();
    crate::toolbar::setup(&mut app);
    let mut review = Review::default();
    if let Some((name, bytes)) = file {
        review.open(&mut app, name, bytes);
    }
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
        |ui, (review, app): &mut (Review, WordApp)| {
            let ctx = ui.ctx().clone();
            review.logic(app);
            app.logic(&ctx);
            review.chrome(ui, app);
            app.ui(ui);
        },
        (review, app),
    );
    for _ in 0..6 {
        h.step();
    }
    h
}

/// How many controls (accessible nodes other than text) are labelled exactly `label`. A card's
/// chip may say "Undo": it names a recorded event, it isn't the command.
fn labelled<S>(h: &egui_kittest::Harness<'static, S>, label: &str) -> usize {
    use egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable};
    h.query_all_by_label(label).filter(|n| !matches!(n.accesskit_node().role(), Role::Label | Role::TextRun | Role::Paragraph)).count()
}

/// The rail's controls and the Navigation pane's (Find): all of them only look.
const RAIL: &[&str] =
    &["First event", "Previous event", "Next event", "Last event", "Event", "Jump to paste", "Go to", "Details", "Headings", "Pages", "Results"];

/// Every control on the page (a node a reader can act on, other than the page itself), with its
/// accessible name.
fn controls<S>(h: &egui_kittest::Harness<'static, S>) -> Vec<(egui::accesskit::Role, Option<String>)> {
    use egui::accesskit::Role;
    use egui_kittest::kittest::NodeT;
    let mut out = Vec::new();
    let mut stack = vec![h.root()];
    while let Some(n) = stack.pop() {
        let a = n.accesskit_node();
        let control = matches!(
            a.role(),
            Role::Button
                | Role::CheckBox
                | Role::Switch
                | Role::RadioButton
                | Role::Tab
                | Role::MenuItem
                | Role::ComboBox
                | Role::Slider
                | Role::SpinButton
                | Role::Link
                | Role::TextInput
        );
        if control {
            out.push((a.role(), a.label()));
        }
        stack.extend(n.children());
    }
    out
}

/// FAL-448: the review page shows the viewing controls and none of Write's editing controls,
/// with or without a file, while the writer's window still has them.
#[test]
fn the_review_page_shows_no_editing_control() {
    use crate::toolbar::{HIDDEN, SHOWN};
    let bytes = written_docx();
    for (file, panes) in [(None, false), (Some(("essay.docx", bytes.as_slice())), false), (Some(("essay.docx", bytes.as_slice())), true)] {
        let mut h = review_page(file);
        if panes {
            // Find (the Navigation pane) and the file's comments.
            for id in ["view.navigationPane", "view.commentsPane"] {
                h.state_mut().1.run(id, json!({"value": true})).unwrap_or_else(|e| panic!("{e}"));
            }
            for _ in 0..4 {
                h.step();
            }
        }
        for name in SHOWN {
            assert!(labelled(&h, name) >= 1, "{name} is missing from the review page (file: {})", file.is_some());
        }
        for name in HIDDEN {
            assert_eq!(labelled(&h, name), 0, "{name} is on the review page (file: {})", file.is_some());
        }
        // Write's quick-access buttons (Undo, Save, AutoSave…) have no accessible name, so a list
        // of names can't miss them: every control on the page must be a viewing one.
        // The Find box has hint text but no name (upstream).
        let strays = controls(&h)
            .into_iter()
            .filter(|(role, l)| match l.as_deref() {
                Some(l) => !(SHOWN.contains(&l) || RAIL.contains(&l)),
                None => !(panes && *role == egui::accesskit::Role::TextInput),
            })
            .collect::<Vec<_>>();
        assert!(strays.is_empty(), "controls that aren't viewing controls (file: {}): {strays:?}", file.is_some());
        let (_, app) = h.state();
        assert!(app.viewer_chrome && app.palette == wordcraft_ui_egui::theme::Palette::Critic);
    }
    // The same names do find Write's controls in an ordinary window, so the check above has teeth.
    let mut app = WordApp::new(Session::new(Document::from_text("one")), Services::default());
    app.ui.language = "en".into();
    let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
        |ui, app: &mut WordApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    for _ in 0..6 {
        h.step();
    }
    let on_write: Vec<&str> = HIDDEN.iter().copied().filter(|n| labelled(&h, n) > 0).collect();
    for name in
        ["File", "Home", "Insert", "Draw", "Design", "Layout", "References", "Mailings", "Review", "View", "Developer", "Zotero", "Editing", "Share"]
    {
        assert!(on_write.contains(&name), "{name} is in Write's window (found: {on_write:?})");
    }
    assert!(!h.state().viewer_chrome && h.state().palette == wordcraft_ui_egui::theme::Palette::WordCraft);
}

/// A viewer's shortcuts can't open editing UI: of the front end's commands only the Zoom and Word
/// Count dialogs run; the view-only session still refuses every edit.
#[test]
fn a_viewer_runs_only_viewing_commands() {
    let bytes = written_docx();
    let mut app = review_app();
    crate::toolbar::setup(&mut app);
    let mut review = Review::default();
    review.open(&mut app, "essay.docx", &bytes);
    let shown = app.session.doc.clone();
    for (id, p) in [
        ("ui.tab", json!({"tab": "Insert"})),
        ("ui.backstage", json!({})),
        ("ui.dialog", json!({"name": "font"})),
        ("ui.dialog", json!({"name": "paragraph"})),
        ("edit.findReplace", json!({})),
        ("text.insert", json!({"text": "x"})),
        ("review.newComment", json!({"text": "note"})),
        ("file.save", json!({"path": "copy.docx"})),
    ] {
        assert!(app.run(id, p.clone()).is_err(), "{id} {p} ran on the review page");
        assert!(app.session.doc.same_content(&shown), "{id} changed the reviewed document");
    }
    for (id, p) in [
        ("ui.dialog", json!({"name": "zoom"})),
        ("view.zoom", json!({"value": "onePage"})),
        ("view.zoomIn", json!({})),
        ("view.commentsPane", json!({"value": true})),
        ("view.navigationPane", json!({"value": true})),
        ("view.darkMode", json!({"value": true})),
    ] {
        assert!(app.run(id, p.clone()).is_ok(), "{id} {p} is refused on the review page");
    }
}

/// The paste's card: its chip, size, times and the technical fields under Details.
#[test]
fn the_paste_card_says_what_the_paste_was() {
    let bytes = written_docx();
    let mut app = review_app();
    let mut review = Review::default();
    review.open(&mut app, "essay.docx", &bytes);
    let paste = review.first_paste().unwrap_or_else(|| panic!("no paste"));
    let l = review.loaded.as_ref().unwrap_or_else(|| panic!("not opened"));
    assert_eq!(crate::rail::observations(l), vec![paste]);
    let c = crate::rail::event_card(l, paste, None).unwrap_or_else(|| panic!("no card"));
    assert_eq!((c.kind, c.label.as_str(), c.seq), (crate::card::Kind::Observation, "Paste", Some(paste)));
    assert_eq!(c.size.as_deref(), Some("26 words · 185 characters added"));
    assert_eq!(c.recorded.as_deref(), Some("28 Oct 2026, 09:00:00 UTC"));
    assert!(c.elapsed.as_deref().is_some_and(|e| e.ends_with("into the session")), "{:?}", c.elapsed);
    assert_eq!(c.excerpt.as_deref(), Some(text_185().as_str()));
    let keys: Vec<&str> = c.details.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["Event", "Origin", "gestureId", "Clipboard formats", "Source"]);
    assert!(c.details.iter().any(|(k, v)| k == "gestureId" && v != "none"));
}

#[test]
fn card_words_are_singular_for_one() {
    use crate::card::count;
    use crate::rail::{elapsed, recorded, size};
    assert_eq!(count(1, "paste", "pastes"), "1 paste");
    assert_eq!(count(0, "paste", "pastes"), "0 pastes");
    assert_eq!(count(2, "paste", "pastes"), "2 pastes");
    assert_eq!(size("a", ""), Some("1 word · 1 character added".into()));
    assert_eq!(size("", "two words"), Some("2 words · 9 characters removed".into()));
    assert_eq!(size("", ""), None);
    assert_eq!(elapsed(65_000), "1:05 into the session");
    assert_eq!(elapsed(3_725_000), "1:02:05 into the session");
    assert_eq!(recorded("2026-10-28T09:00:00Z"), "28 Oct 2026, 09:00:00 UTC");
    assert_eq!(recorded("not a time"), "not a time");
    assert_eq!(crate::card::excerpt(&format!("one{}two", wordcraft_inkwitness::PARA_MARK)), "“one\ntwo”");
}
