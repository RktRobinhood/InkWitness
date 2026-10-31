//! The sample submission (FAL-519): a made-up essay by "Example Student", written through the
//! real writer ([`crate::writer`]) with a scripted clock, so the review page can be shown without
//! a real pupil's file. Everything in it is synthetic (`AGENTS.md`: no pupil data).
//!
//! The script types at a person's pace, pauses to think, pastes a short quotation, pastes a long
//! chatbot answer and trims its wrapper, then writes a conclusion, deletes it and writes it
//! again. That gives every flag a teacher is shown except "Text already there".

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use wordcraft_doc::{Document, Path, Pos, StoryRef};
use wordcraft_engine::{Selection, Session};
use wordcraft_inkwitness::Env;
use wordcraft_ui_egui::{Services, WordApp};

/// The sample's file name.
pub const NAME: &str = "Example Student - TOK essay (sample).docx";

/// When the sample session starts.
const START: &str = "2026-10-27T13:05:00Z";

/// The scripted clock: the script moves it on, the recorder reads it.
struct Clock {
    ms: Arc<AtomicU64>,
    ids: u64,
}

impl Env for Clock {
    fn now_iso(&mut self) -> String {
        let ms = self.ms.load(Ordering::Relaxed);
        chrono::DateTime::parse_from_rfc3339(START)
            .map(|t| (t + chrono::Duration::milliseconds(i64::try_from(ms).unwrap_or(0))).to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
            .unwrap_or_else(|_| START.to_string())
    }
    fn monotonic_ms(&mut self) -> u64 {
        self.ms.load(Ordering::Relaxed)
    }
    fn uuid(&mut self) -> String {
        self.ids += 1;
        let mut b = [0x5Au8; 16];
        b[8..].copy_from_slice(&self.ids.to_be_bytes());
        wordcraft_inkwitness::uuid_v4(b)
    }
}

const OPENING: &str = "To what extent does the way we gather knowledge shape what we accept as true? In this essay I look at two areas of knowledge, history and the natural sciences, and ask how each decides when a claim has enough evidence behind it.";
const HISTORY_START: &str = "Historians rarely have the full picture. As E. H. Carr put it, ";
const CARR: &str = "\u{201C}the facts speak only when the historian calls on them\u{201D}";
const HISTORY_END: &str =
    ". The historian\u{2019}s choices are part of the knowledge itself, which is why two honest accounts of the same event can disagree.";
const CHATBOT: &str = "Certainly! Here\u{2019}s a paragraph about the natural sciences:\nIn the natural sciences, knowledge is built through repeated observation and controlled experiment. A hypothesis is accepted only when independent researchers can reproduce its results, and even then it remains open to revision. This self-correcting method gives scientific claims a different kind of authority from historical interpretation, because the evidence can, in principle, be checked again by anyone.\nI hope this helps! Let me know if you\u{2019}d like me to expand on any point.";
const FIRST_CONCLUSION: &str = "In conclusion, both areas of knowledge are shaped by the methods used to gather it. History depends on the judgement of the historian, while science depends on whether results can be repeated by others.";
const CONCLUSION: &str = "So the way we gather knowledge does shape what we accept: a historian argues for an interpretation, while a scientist waits for others to repeat the result.";

/// The script's state: the app it writes in, its clock and a small random source for pauses.
struct Script {
    app: WordApp,
    ms: Arc<AtomicU64>,
    seed: u64,
}

impl Script {
    fn wait(&mut self, ms: u64) {
        self.ms.fetch_add(ms, Ordering::Relaxed);
    }

    /// A number in `lo..hi` (a fixed sequence: the sample is the same every time).
    fn between(&mut self, lo: u64, hi: u64) -> u64 {
        self.seed = self.seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        lo + (self.seed >> 33) % (hi - lo).max(1)
    }

    fn run(&mut self, id: &str, params: Value) {
        // The script's commands are all valid; one that fails leaves the sample a little shorter.
        let _ = self.app.run(id, params);
    }

    /// Type `text` one character at a time, a little faster within words, with a pause after
    /// each sentence.
    fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            let gap = if c == ' ' { self.between(140, 320) } else { self.between(90, 230) };
            self.wait(gap);
            self.run("text.insert", json!({"text": c.to_string()}));
            if matches!(c, '.' | '?' | '!') {
                let think = self.between(1_200, 3_500);
                self.wait(think);
            }
        }
    }

    fn new_paragraph(&mut self) {
        self.wait(600);
        self.run("text.newParagraph", json!({}));
    }

    fn paste(&mut self, text: &str) {
        self.wait(1_400);
        self.run("edit.paste", json!({"text": text}));
    }

    /// The body paragraph whose text starts with `prefix`: its index and length.
    fn para(&self, prefix: &str) -> Option<(u32, usize)> {
        self.app.session.doc.body.iter().enumerate().find_map(|(i, b)| {
            let p = b.as_para().filter(|p| p.text.starts_with(prefix))?;
            Some((u32::try_from(i).ok()?, p.text.len()))
        })
    }

    /// Select from `a` to `b` and delete it.
    fn delete(&mut self, a: (u32, usize), b: (u32, usize)) {
        let at = |(i, off): (u32, usize)| Pos::new(StoryRef::Body, Path(vec![i]), off);
        let think = self.between(1_500, 3_000);
        self.wait(think);
        self.app.session.sel = Selection { anchor: at(a), focus: at(b) };
        self.run("text.delete", json!({}));
    }

    fn caret_to_end(&mut self) {
        self.run("caret.docEnd", json!({}));
    }
}

/// The sample submission, as InkWitness Write saves it: its name and DOCX bytes.
pub fn docx() -> (String, Vec<u8>) {
    docx_with(true)
}

/// The same session recorded one event per key press, as a version 1.0 writer did: the old-file
/// fixture the review page still has to read.
pub fn docx_per_letter() -> (String, Vec<u8>) {
    docx_with(false)
}

fn docx_with(merge_typing: bool) -> (String, Vec<u8>) {
    let ms = Arc::new(AtomicU64::new(0));
    let mut app = WordApp::new(Session::new(Document::new()), Services::default());
    let who = crate::writer::Identity { student: "Example Student".into(), assignment: "TOK essay (sample)".into() };
    let recorder = crate::writer::attach_for(&mut app, Box::new(Clock { ms: ms.clone(), ids: 0 }), Box::new(|| START.to_string()), Some(who));
    recorder.set_merge_typing(merge_typing);
    let mut s = Script { app, ms, seed: 0x1D_u64 };
    s.wait(4_000);
    s.type_text(OPENING);
    s.new_paragraph();
    s.type_text(HISTORY_START);
    s.paste(CARR);
    s.type_text(HISTORY_END);
    // Three minutes away from the page.
    s.wait(184_000);
    s.new_paragraph();
    s.paste(CHATBOT);
    // The chatbot's opening line and its sign-off go; the paragraph between stays.
    if let (Some(first), Some(body)) = (s.para("Certainly"), s.para("In the natural sciences")) {
        s.delete((first.0, 0), (body.0, 0));
    }
    if let (Some(body), Some(last)) = (s.para("In the natural sciences"), s.para("I hope this helps")) {
        s.delete((body.0, body.1), (last.0, last.1));
    }
    s.wait(42_000);
    s.caret_to_end();
    s.new_paragraph();
    s.type_text(FIRST_CONCLUSION);
    s.wait(25_000);
    if let Some(c) = s.para("In conclusion") {
        s.delete((c.0, 0), (c.0, c.1));
    }
    s.type_text(CONCLUSION);
    s.wait(3_000);
    let bytes = save(&s.app).unwrap_or_default();
    (NAME.to_string(), bytes)
}

/// Save the way the writer does: the history goes into the DOCX.
fn save(app: &WordApp) -> Option<Vec<u8>> {
    let prepare = app.services.prepare_save.as_ref()?;
    let doc = prepare(NAME, &app.session.doc).ok()??;
    wordcraft_engine::io::save_bytes(NAME, &doc).ok()
}
