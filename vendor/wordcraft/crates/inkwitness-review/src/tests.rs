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
    saved(&app)
}

/// The DOCX the writer page saves for `app`'s document.
fn saved(app: &WordApp) -> Vec<u8> {
    let prepare = app.services.prepare_save.as_ref().unwrap_or_else(|| panic!("no save hook"));
    let doc = prepare("essay.docx", &app.session.doc).unwrap_or_else(|e| panic!("{e}")).unwrap_or_else(|| panic!("nothing embedded"));
    assert!(prepare("essay.pdf", &app.session.doc).unwrap_or_else(|e| panic!("{e}")).is_none(), "only a Word document carries the history");
    wordcraft_engine::io::save_bytes("essay.docx", &doc).unwrap_or_else(|e| panic!("{e}"))
}

/// A writer where the student picks New (`params`) on the start screen, then types "Test 123"
/// (FAL-547); the DOCX bytes.
fn written_after_new(params: Value) -> Vec<u8> {
    let mut app = WordApp::new(Session::new(Document::new()), Services::default());
    super::writer::attach(&mut app, Box::new(TestEnv { ms: 0, ids: 0 }), Box::new(|| "2026-10-28T09:05:00Z".to_string()));
    run(&mut app, "file.new", params);
    for c in "Test 123".chars() {
        run(&mut app, "text.insert", json!({"text": c.to_string()}));
    }
    saved(&app)
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

/// The rail's, the player dock's and the Navigation pane's (Find) controls: they look, play, or
/// write the teacher's own comments, which never enter the student's file. The rail's two tabs
/// carry a count ("Flags (3)").
const RAIL: &[&str] = &[
    "Try a sample submission",
    "Go to",
    "Promote to comment",
    "Details",
    "Comment text",
    "Save comment",
    "Discard",
    "Show",
    "Edit",
    "Remove",
    "Download review copy (.docx)",
    "Previous flag",
    "Previous event",
    "Play",
    "Pause",
    "Next event",
    "Next flag",
    "+ Add comment",
    "Pause at flags",
    "Skip idle",
    "Playback speed",
    "Replay timeline",
    "Headings",
    "Pages",
    "Results",
];

/// Whether `name` is one of the rail's or the dock's controls ([`RAIL`]).
fn on_rail(name: &str) -> bool {
    RAIL.contains(&name) || name.starts_with("Flags (") || name.starts_with("Comments (")
}

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
                Some(l) => !(SHOWN.contains(&l) || on_rail(l)),
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

/// FAL-448: the rulers' indent, margin and tab-stop markers edit the page, and they are painted,
/// not accessible controls, so the test above can't see them. The page must not make room for a
/// ruler however `view.ruler` gets turned on (a stored preference, the command, a shortcut),
/// while the same window without the viewer chrome does.
#[test]
fn the_review_page_draws_no_ruler() {
    let bytes = written_docx();
    let mut h = review_page(Some(("essay.docx", bytes.as_slice())));
    let canvas = |h: &mut egui_kittest::Harness<'static, (Review, WordApp)>| {
        for _ in 0..4 {
            h.step();
        }
        h.state().1.canvas.canvas_rect.unwrap_or_else(|| panic!("the page is shown"))
    };
    h.state_mut().1.session.view.ruler = false;
    let bare = canvas(&mut h);
    h.state_mut().1.session.view.ruler = true;
    assert_eq!(canvas(&mut h), bare, "a ruler is drawn on the review page (view.ruler set)");
    h.state_mut().1.session.view.ruler = false;
    h.state_mut().1.run("view.ruler", json!({"value": true})).unwrap_or_else(|e| panic!("{e}"));
    assert!(h.state().1.session.view.ruler);
    assert_eq!(canvas(&mut h), bare, "a ruler is drawn on the review page (view.ruler run)");
    // The check has teeth: without the viewer chrome, the ruler takes its strip off the page.
    h.state_mut().1.viewer_chrome = false;
    let with = canvas(&mut h);
    h.state_mut().1.session.view.ruler = false;
    let without = canvas(&mut h);
    assert!(with.min.x > without.min.x && with.min.y > without.min.y, "the ruler isn't drawn in Write either ({with:?} vs {without:?})");
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
    assert_eq!(size(" ", ""), Some("1 character added".into()));
    assert_eq!(crate::card::excerpt("“the facts speak”"), "“the facts speak”");
    assert_eq!(crate::card::excerpt("\"quoted\" and more"), "“\"quoted\" and more”");
    assert_eq!(elapsed(65_000), "1:05 into the session");
    assert_eq!(elapsed(3_725_000), "1:02:05 into the session");
    assert_eq!(recorded("2026-10-28T09:00:00Z"), "28 Oct 2026, 09:00:00 UTC");
    assert_eq!(recorded("not a time"), "not a time");
    assert_eq!(crate::card::excerpt(&format!("one{}two", wordcraft_inkwitness::PARA_MARK)), "“one\ntwo”");
}

/// FAL-519: the sample submission is a real recorded history that replays, and every flag the
/// demo shows is in it.
#[test]
fn the_sample_submission_replays_with_its_flags() {
    use crate::flags::FlagKind;
    let t = std::time::Instant::now();
    let (name, bytes) = crate::sample::docx();
    let made = t.elapsed();
    let l = super::review::load(&name, &bytes).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(l.consistency, Consistency::Consistent, "{:?}", l.consistency);
    assert_eq!(l.student.as_deref(), Some("Example Student"));
    let flags = crate::flags::compute(&l);
    let kinds: Vec<FlagKind> = flags.iter().map(|f| f.kind).collect();
    for k in [FlagKind::Paste, FlagKind::LargePaste, FlagKind::AiPhrase, FlagKind::LargeDelete] {
        assert!(kinds.contains(&k), "{k:?} is missing: {flags:?}");
    }
    let ai = flags.iter().find(|f| f.kind == FlagKind::AiPhrase).unwrap_or_else(|| panic!("no AI-style phrase"));
    assert_eq!(ai.matched.as_deref(), Some("Certainly! Here"));
    // The chatbot's wrapper was trimmed: the final text keeps only the paragraph between.
    let text = l.last.doc.plain_text(StoryRef::Body);
    assert!(text.contains("In the natural sciences") && !text.contains("I hope this helps") && !text.contains("Certainly"), "{text}");
    assert!(made.as_secs_f32() < 5.0, "the sample takes {made:?} to make");
}

/// The text between comment `id`'s start and end in `doc`'s body.
fn commented(doc: &Document, id: u32) -> Option<String> {
    use wordcraft_doc::InlineObject;
    use wordcraft_doc::para::OBJ;
    let mut out: Option<String> = None;
    for b in &doc.body {
        let Some(p) = b.as_para() else { continue };
        let mut objs = p.objects.iter();
        for c in p.text.chars() {
            if c == OBJ {
                match objs.next() {
                    Some(InlineObject::CommentStart { id: i }) if *i == id => out = Some(String::new()),
                    Some(InlineObject::CommentEnd { id: i }) if *i == id => return out,
                    _ => {}
                }
            } else if let Some(o) = out.as_mut() {
                o.push(c);
            }
        }
        if let Some(o) = out.as_mut() {
            o.push('\n');
        }
    }
    out
}

/// FAL-519: Promote to comment on the AI-style phrase flag opens an editable draft from the
/// record; the trimmed text is saved as a teacher comment, drawn on the page on the pasted
/// passage, while the replayed state and the highlight stay exactly as recorded.
#[test]
fn a_promoted_flag_becomes_an_edited_comment_on_the_page() {
    use crate::flags::FlagKind;
    let (name, bytes) = crate::sample::docx();
    let mut app = review_app();
    let mut review = Review::default();
    review.open(&mut app, &name, &bytes);
    let seq = review.flags.iter().find(|f| f.kind == FlagKind::AiPhrase).map(|f| f.seq).unwrap_or_else(|| panic!("no flag"));
    review.promote(&mut app, seq);
    let d = review.draft.clone().unwrap_or_else(|| panic!("no draft"));
    assert!(d.text.contains("“Certainly! Here”") && d.text.trim_end().ends_with('?'), "{}", d.text);
    assert!(d.anchor.as_ref().is_some_and(|a| a.quote.starts_with("In the natural sciences")), "anchored on what survives: {:?}", d.anchor);
    // The teacher trims the draft to its first line and adds a word.
    let first = d.text.lines().next().unwrap_or_default().to_string();
    if let Some(dr) = review.draft.as_mut() {
        dr.text = format!("{first} Let's talk about this paragraph.");
    }
    review.save_draft();
    assert!(review.draft.is_none() && review.comments.len() == 1 && review.promoted(seq));
    review.logic(&mut app);
    let l = review.loaded.as_ref().unwrap_or_else(|| panic!("loaded"));
    assert!(l.state.doc.comments.is_empty(), "the replayed state never gets the comment");
    let (id, c) = app.session.doc.comments.iter().next().unwrap_or_else(|| panic!("no comment on the page"));
    assert_eq!(c.author, crate::export::AUTHOR);
    let on = commented(&app.session.doc, *id).unwrap_or_default();
    assert!(on.starts_with("In the natural sciences"), "{on}");
    // The page's highlight is still exactly the event's text.
    let (a, b) = app.canvas.review_mark.clone().unwrap_or_else(|| panic!("no highlight"));
    let hi = review.highlighted_text().unwrap_or_default();
    let on_page = wordcraft_inkwitness::replay::State::unrecorded(app.session.doc.clone()).text(&a, &b).replace(wordcraft_doc::para::OBJ, "");
    assert_eq!(on_page.replace(['\n', wordcraft_inkwitness::PARA_MARK], ""), hi.replace('\n', ""));
}

/// FAL-519: a comment on text selected on the page (offsets shifted by earlier comments) lands
/// on that text, and the review copy carries both as Word comments; the student's file is not
/// touched.
#[test]
fn comments_on_selected_text_go_into_the_review_copy() {
    let (name, bytes) = crate::sample::docx();
    let mut app = review_app();
    let mut review = Review::default();
    review.open(&mut app, &name, &bytes);
    let last = review.loaded.as_ref().map_or(0, |l| l.replayable);
    let comment_on = |review: &mut Review, app: &mut WordApp, word: &str, text: &str| {
        review.logic(app);
        let doc = &app.session.doc;
        let (k, p) = doc
            .body
            .iter()
            .enumerate()
            .find_map(|(k, b)| b.as_para().filter(|p| p.text.contains(word)).map(|p| (k, p)))
            .unwrap_or_else(|| panic!("{word}"));
        let at = p.text.find(word).unwrap_or(0);
        let pos = |off| wordcraft_doc::Pos::new(StoryRef::Body, wordcraft_doc::Path(vec![u32::try_from(k).unwrap_or(0)]), off);
        app.session.sel = wordcraft_engine::Selection { anchor: pos(at), focus: pos(at + word.len()) };
        review.comment_here(app);
        if let Some(d) = review.draft.as_mut() {
            d.text = text.into();
        }
        review.save_draft();
    };
    review.go(&mut app, last);
    comment_on(&mut review, &mut app, "two honest accounts", "Good point: give an example.");
    // Same paragraph, after the first comment's marks.
    comment_on(&mut review, &mut app, "the same event", "Which event?");
    assert_eq!(review.comments.len(), 2);
    let quotes: Vec<&str> = review.comments.iter().filter_map(|c| c.anchor.as_ref().map(|a| a.quote.as_str())).collect();
    assert_eq!(quotes, ["two honest accounts", "the same event"]);
    let l = review.loaded.as_ref().unwrap_or_else(|| panic!("loaded"));
    let (copy, out) = crate::export::review_copy(l, &review.comments).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(copy, "Example Student - TOK essay (sample) (reviewed).docx");
    let doc = wordcraft_engine::io::open_bytes(&copy, &out).unwrap_or_else(|e| panic!("{e}"));
    let mut found: Vec<String> = doc.comments.keys().filter_map(|id| commented(&doc, *id)).collect();
    found.sort();
    assert_eq!(found, ["the same event", "two honest accounts"]);
    assert!(wordcraft_inkwitness::envelope::extract(&doc).ok().flatten().is_none(), "the review copy carries no history");
    // The student's file is what it was.
    let again = super::review::load(&name, &bytes).unwrap_or_else(|e| panic!("{e}"));
    assert!(again.last.doc.comments.is_empty());
}

/// FAL-519: + Add comment opens a draft whose text box has the keyboard, so typing goes into the
/// comment, not to the page or the player's shortcuts (Space plays and pauses).
#[test]
fn a_new_comment_draft_has_the_keyboard() {
    use egui_kittest::kittest::Queryable;
    let (name, bytes) = crate::sample::docx();
    let mut h = review_page(Some((&name, &bytes)));
    let seq = h.state().0.loaded.as_ref().map_or(0, |l| l.state.seq);
    h.get_by_label("+ Add comment").click();
    for _ in 0..4 {
        h.step();
    }
    assert!(h.state().0.draft.is_some(), "a draft is open");
    assert!(h.state().0.typing, "the draft's text box has the keyboard");
    h.get_by_label("Comment text").type_text("Which event? ");
    for _ in 0..3 {
        h.step();
    }
    let (review, _) = h.state();
    assert_eq!(review.draft.as_ref().map(|d| d.text.as_str()), Some("Which event? "));
    assert_eq!(review.loaded.as_ref().map_or(0, |l| l.state.seq), seq, "typing never moved the replay");
    assert!(!review.player.playing);
}

/// The New picked on the writer's start screen is not a section of the timeline before the
/// writing, and a blank one is not "Text already there": Play starts on the student's first
/// change (FAL-547).
#[test]
fn the_review_starts_where_the_writing_starts() {
    use crate::flags::{self, FlagKind};
    use crate::player::{Player, Timeline};
    let new_seq = |l: &super::review::Loaded| l.replay.events().iter().position(|e| e["type"] == "import_document").map(|i| i as u64 + 1);

    let l = super::review::load("essay.docx", &written_after_new(json!({}))).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(Some(l.first), new_seq(&l), "the writing starts after the New");
    assert!(flags::compute(&l).is_empty(), "a blank New put no text there");
    let tl = Timeline::new(l.replay.events(), l.first, l.replayable, true);
    assert_eq!(tl.time_of(l.first), 0.0, "nothing before the writing takes room on the timeline");
    assert!(tl.time_of(l.first + 1) > 0.0);
    assert_eq!(Player::default().play(&tl, l.replayable, l.replayable), Some(l.first), "Play from the end starts where the writing starts");
    assert_eq!(Player::default().play(&tl, 0, l.replayable), Some(l.first), "and from before it too");

    // A document with text in it, opened at the start: playback starts on it, and the flag says
    // how much was already there.
    let l = super::review::load("essay.docx", &written_after_new(json!({"template": "sample"}))).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(Some(l.first), new_seq(&l));
    let f = flags::compute(&l);
    let opened: Vec<_> = f.iter().filter(|f| f.kind == FlagKind::Imported).collect();
    assert_eq!(opened.len(), 1, "{f:?}");
    assert_eq!(Some(opened[0].seq), new_seq(&l));
    assert!(!opened[0].label.ends_with(" 0 characters"), "{}", opened[0].label);

    // Typing first: the writing starts at the start.
    let l = super::review::load("essay.docx", &written_docx()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(l.first, 0);
}

/// A 1×1 PNG.
const PNG: [u8; 69] = [
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65,
    84, 120, 156, 99, 56, 33, 39, 7, 0, 2, 182, 1, 5, 52, 166, 117, 170, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

/// A picture the student inserted shows on the review page whenever it's in the shown state, and
/// is in the review copy: the history says where it goes, and the saved file has the picture
/// (FAL-547).
#[test]
fn pictures_come_from_the_saved_file() {
    let mut app = WordApp::new(Session::new(Document::new()), Services::default());
    super::writer::attach(&mut app, Box::new(TestEnv { ms: 0, ids: 0 }), Box::new(|| "2026-10-28T09:05:00Z".to_string()));
    run(&mut app, "text.insert", json!({"text": "A"}));
    run(&mut app, "insert.picture", json!({"data": wordcraft_engine::cmd::insert::base64_encode(&PNG)}));
    let key = app.session.doc.media.keys().next().cloned().unwrap_or_else(|| panic!("no picture inserted"));
    let bytes = saved(&app);

    let mut app = review_app();
    let mut review = Review::default();
    review.open(&mut app, "essay.docx", &bytes);
    let l = review.loaded.as_ref().unwrap_or_else(|| panic!("{:?}", review.error));
    assert_eq!(l.consistency, Consistency::Consistent);
    assert!(!l.state.doc.media.contains_key(&key), "the history alone has no picture");
    let shown = |app: &WordApp| app.session.doc.media.get(&key).map(|b| b.to_vec());
    assert_eq!(shown(&app), Some(PNG.to_vec()), "the last state shows it");
    review.seek(&mut app, 1);
    review.seek(&mut app, 2);
    assert_eq!(shown(&app), Some(PNG.to_vec()), "and still does when the reviewer comes back to it");

    let l = review.loaded.as_ref().unwrap_or_else(|| panic!("no file"));
    let (name, copy) = crate::export::review_copy(l, &[]).unwrap_or_else(|e| panic!("{e}"));
    let reopened = wordcraft_engine::io::open_bytes(&name, &copy).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(reopened.media.values().map(|b| b.to_vec()).collect::<Vec<_>>(), vec![PNG.to_vec()], "the review copy has it");
}
