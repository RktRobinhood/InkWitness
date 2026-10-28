//! The `/review/` route (G0 spike): open a DOCX with an InkWitness history from a local file,
//! rebuild any recorded state in WordCraft's page renderer, read-only, and step or seek through
//! the events. Seeking to an event highlights its range on the page and scrolls it into view;
//! a side note shows the event. The full video-style transport is G2.
//!
//! Nothing is uploaded: the file arrives through the app's inbox (the browser's file picker or a
//! drop) and stays in memory. The student's file is never written: the session is view-only
//! ([`wordcraft_engine::Session::set_read_only`]) and the shown document is put back if anything
//! changes it.

use serde_json::Value;
use wordcraft_doc::{Document, StoryRef};
use wordcraft_engine::{Selection, Session};
use wordcraft_inkwitness::envelope;
use wordcraft_inkwitness::replay::{Replay, ReplayError, Resolved, State};
use wordcraft_ui_egui::WordApp;

/// Largest file the review page reads.
pub const MAX_FILE_BYTES: usize = 64 << 20;

/// The text the review page starts with.
pub const START_TEXT: &str = "InkWitness Review (G0 technical preview). Open a DOCX saved by InkWitness Write: use Open DOCX in the panel on the right, or drop the file on this page. The file stays on this computer.";

/// What the history in a file says about itself (`AGENTS.md`: local consistency, external
/// changes and corruption are told apart; none of this is a signature).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Consistency {
    /// Every event replays, each to its recorded digest, ending at the recorded final state, and
    /// the file's text is that state's text.
    Consistent,
    /// The history replays, but the file's text differs from its last recorded state: it was
    /// changed where InkWitness didn't see it.
    ExternallyChanged,
    /// The history stops replaying at this event.
    Corrupt(String),
    /// The file has no InkWitness history.
    Missing,
}

impl Consistency {
    pub fn label(&self) -> String {
        match self {
            Consistency::Consistent => {
                "Local history consistent: every event rebuilds the recorded document (unsigned; not proof of authorship)".into()
            }
            Consistency::ExternallyChanged => "External changes: the file's text differs from its last recorded state".into(),
            Consistency::Corrupt(why) => format!("History corrupt: {why}"),
            Consistency::Missing => "History missing: this file has no InkWitness edit history".into(),
        }
    }
}

/// A file opened for review.
pub struct Loaded {
    pub name: String,
    pub replay: Replay,
    pub state: State,
    pub consistency: Consistency,
    /// The last event the history replays to (all of them when it is consistent).
    pub replayable: u64,
    /// The shown event's range in the shown state, if it has one.
    pub mark: Option<Resolved>,
}

/// The review page's state.
#[derive(Default)]
pub struct Review {
    pub loaded: Option<Loaded>,
    /// Why the last file couldn't be opened.
    pub error: Option<String>,
    /// The status line last published ([`Review::status_changed`]).
    published: String,
}

/// A view-only session showing the start text on `doc`'s page.
pub fn start_session(mut doc: Document) -> Session {
    let para = wordcraft_doc::Paragraph::with_text(START_TEXT, wordcraft_doc::CharProps::default());
    doc.body = vec![std::sync::Arc::new(wordcraft_doc::Block::Para(para))];
    let mut s = Session::new(doc);
    s.set_read_only(true);
    s
}

impl Review {
    /// Take the files waiting in the app's inbox (picker, drop) for review, so the app never
    /// opens one as a document to edit. The last one is reviewed.
    pub fn take_files(&mut self, app: &mut WordApp) {
        let files = match &app.services.inbox {
            Some(inbox) => std::mem::take(&mut *inbox.lock().unwrap_or_else(std::sync::PoisonError::into_inner)),
            None => return,
        };
        if let Some((name, bytes)) = files.into_iter().last() {
            self.open(app, &name, &bytes);
        }
    }

    /// Open `bytes` (a DOCX) for review and show its last replayable state.
    pub fn open(&mut self, app: &mut WordApp, name: &str, bytes: &[u8]) {
        match load(name, bytes) {
            Ok(l) => {
                self.error = None;
                let last = l.replayable;
                self.loaded = Some(l);
                self.seek(app, last);
            }
            Err(e) => {
                app.status(e.clone());
                self.error = Some(e);
            }
        }
    }

    /// Show the state after event `seq` (clamped to the replayable ones), its range
    /// highlighted and scrolled into view.
    pub fn seek(&mut self, app: &mut WordApp, seq: u64) {
        let Some(l) = &mut self.loaded else { return };
        let seq = seq.min(l.replayable);
        let next = if seq >= l.state.seq {
            let mut st = l.state.clone();
            l.replay.advance(&mut st, seq).map(|()| st)
        } else {
            l.replay.state_at(seq)
        };
        match next {
            Ok(st) => l.state = st,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        }
        l.mark = l.replay.event(l.state.seq).and_then(|e| event_range(&l.state, e));
        show(app, l);
    }

    /// The first paste event, if there is one.
    pub fn first_paste(&self) -> Option<u64> {
        self.loaded.as_ref()?.replay.pastes().into_iter().find(|&s| self.loaded.as_ref().is_some_and(|l| s <= l.replayable))
    }

    /// Per frame, before the app's own logic: take new files, and put the shown document back
    /// if anything changed it (the session refuses edits; a change made around it is undone).
    pub fn logic(&mut self, app: &mut WordApp) {
        self.take_files(app);
        if let Some(l) = &self.loaded
            && !app.session.doc.same_content(&l.state.doc)
        {
            show(app, l);
        }
    }

    /// The highlighted text of the shown event (paragraph marks as line breaks).
    pub fn highlighted_text(&self) -> Option<String> {
        let l = self.loaded.as_ref()?;
        let m = l.mark.as_ref()?;
        Some(l.state.text(&m.start, &m.end).replace(wordcraft_inkwitness::PARA_MARK, "\n"))
    }

    /// The side note's lines for the shown event.
    pub fn note(&self) -> Vec<String> {
        let Some(l) = &self.loaded else { return Vec::new() };
        let Some(e) = l.replay.event(l.state.seq) else {
            return vec!["Before the first recorded event".into()];
        };
        let s = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or("—").to_string();
        let chars = |k: &str| e.get(k).and_then(Value::as_str).map_or(0, |t| t.chars().filter(|c| *c != wordcraft_inkwitness::PARA_MARK).count());
        let mut out = vec![
            format!("Event {} of {}", l.state.seq, l.replay.last_seq()),
            format!("Type: {}", s("type")),
            format!("Origin: {}", s("origin")),
            format!("Time: {} (+{} ms)", s("atLocalIso"), e.get("elapsedMonotonicMs").and_then(Value::as_u64).unwrap_or(0)),
            format!("Inserted: {} characters, deleted: {}", chars("insertedText"), chars("deletedText")),
            format!("gestureId: {}", e.get("gestureId").and_then(Value::as_str).unwrap_or("none")),
        ];
        if s("type") == "paste" {
            out.push(format!("Clipboard formats: {}", e.get("clipboardTypes").map_or("not exposed by the browser".to_string(), Value::to_string)));
            out.push("Source: unknown (a browser can't tell where pasted text came from)".into());
        }
        match &l.mark {
            Some(m) if m.context_matches => out.push("Anchor: resolved, surrounding text matches".into()),
            Some(_) => out.push("Anchor: resolved, surrounding text not checked or differs (uncertain)".into()),
            None => out.push("Anchor: nothing to highlight in this state".into()),
        }
        out
    }

    /// One line for the page's live status region; also what the browser test reads.
    pub fn status_line(&self) -> String {
        match (&self.loaded, &self.error) {
            (None, Some(e)) => format!("Couldn't open the file: {e}"),
            (None, None) => "No file open".into(),
            (Some(l), err) => {
                let mut s = format!("{} — {}. {}.", l.name, l.consistency.label(), self.note().join("; "));
                if let Some(e) = err {
                    s.push_str(&format!(" {e}."));
                }
                s
            }
        }
    }

    /// The status line, when it changed since it was last asked for.
    pub fn status_changed(&mut self) -> Option<String> {
        let s = self.status_line();
        (s != self.published).then(|| {
            self.published = s.clone();
            s
        })
    }

    /// The review panel, at the right of the page.
    pub fn panel(&mut self, ui: &mut egui::Ui, app: &mut WordApp) {
        egui::Panel::right("inkwitness_review").default_size(330.0).resizable(true).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.panel_contents(ui, app));
        });
    }

    fn panel_contents(&mut self, ui: &mut egui::Ui, app: &mut WordApp) {
        ui.heading("InkWitness Review");
        ui.label("G0 technical preview. Read-only: nothing here changes the student's file.");
        ui.horizontal(|ui| {
            if ui.button("Open DOCX…").clicked()
                && let Some(open) = &app.services.open_async
            {
                open("document");
            }
            ui.label("or drop it on the page");
        });
        if let Some(e) = &self.error {
            ui.colored_label(egui::Color32::from_rgb(190, 40, 40), e);
        }
        let mut go: Option<u64> = None;
        let paste = self.first_paste();
        if let Some(l) = &self.loaded {
            ui.separator();
            ui.strong(&l.name);
            ui.label(l.consistency.label());
            ui.label(format!("{} recorded events, {} pastes", l.replay.last_seq(), l.replay.pastes().len()));
            ui.separator();
            let seq = l.state.seq;
            ui.horizontal(|ui| {
                if ui.button("⏮").on_hover_text("First event").clicked() {
                    go = Some(0);
                }
                if ui.button("◀ Previous").clicked() {
                    go = Some(seq.saturating_sub(1));
                }
                if ui.button("Next ▶").clicked() {
                    go = Some(seq + 1);
                }
                if ui.button("⏭").on_hover_text("Last event").clicked() {
                    go = Some(l.replayable);
                }
            });
            let mut at = seq;
            if ui.add(egui::Slider::new(&mut at, 0..=l.replayable).text("event")).changed() {
                go = Some(at);
            }
            if ui.add_enabled(paste.is_some(), egui::Button::new("Jump to paste")).clicked() {
                go = paste;
            }
            ui.separator();
            egui::Frame::group(ui.style()).show(ui, |ui| {
                for line in self.note() {
                    ui.label(line);
                }
            });
            if let Some(t) = self.highlighted_text() {
                ui.label("Highlighted on the page:");
                ui.label(egui::RichText::new(t).italics());
            }
        }
        if let Some(seq) = go {
            self.seek(app, seq);
        }
    }
}

/// Read a DOCX's history and check it end to end.
pub fn load(name: &str, bytes: &[u8]) -> Result<Loaded, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(format!("{name} is larger than {} MB", MAX_FILE_BYTES >> 20));
    }
    let doc = wordcraft_engine::io::open_bytes(name, bytes).map_err(|e| format!("{name}: {e}"))?;
    let env = match envelope::extract(&doc) {
        Ok(Some(env)) => env,
        Ok(None) => return Err(format!("{name}: {}", Consistency::Missing.label())),
        Err(e) => return Err(format!("{name}: {}", Consistency::Corrupt(e.to_string()).label())),
    };
    let evidence: Value = serde_json::from_slice(&env.payload).map_err(|e| format!("{name}: {}", Consistency::Corrupt(e.to_string()).label()))?;
    let replay = Replay::from_evidence(&evidence).map_err(|e| format!("{name}: {}", Consistency::Corrupt(e.to_string()).label()))?;
    let start = replay.state_at(0).map_err(|e| format!("{name}: {}", Consistency::Corrupt(e.to_string()).label()))?;
    // Replay everything once: the furthest it gets is as far as the reviewer can seek.
    let (consistency, state) = match replay.verify_all() {
        Ok(end) => {
            let same_text = end.doc.plain_text(StoryRef::Body) == doc.plain_text(StoryRef::Body);
            (if same_text { Consistency::Consistent } else { Consistency::ExternallyChanged }, end)
        }
        Err(e) => {
            let stop = match &e {
                ReplayError::Op { seq, .. } | ReplayError::Digest { seq } | ReplayError::Unsupported { seq, .. } | ReplayError::NoCheckpoint(seq) => {
                    seq.saturating_sub(1)
                }
                _ => 0,
            };
            let st = replay.state_at(stop).unwrap_or(start);
            (Consistency::Corrupt(e.to_string()), st)
        }
    };
    let replayable = state.seq;
    Ok(Loaded { name: name.to_string(), replay, state, consistency, replayable, mark: None })
}

/// The range event `e` left in `st` (its after-anchor), checked against the text around it.
pub fn event_range(st: &State, e: &Value) -> Option<Resolved> {
    let window: Vec<String> =
        e.pointer("/ops/0/afterBlocks")?.as_array()?.iter().filter_map(|b| b.get("blockId")?.as_str().map(str::to_string)).collect();
    st.resolve(e.get("afterAnchor")?, Some(&window))
}

/// Put the loaded state on the page: the document, the highlight, the caret at its start and
/// the view following it.
fn show(app: &mut WordApp, l: &Loaded) {
    app.session.doc = l.state.doc.clone();
    app.session.touch();
    app.session.dirty = false;
    let at = l.mark.as_ref().map_or_else(|| app.session.doc.start_of(StoryRef::Body), |m| m.start.clone());
    app.session.sel = Selection::caret(app.session.doc.clamp(&at));
    app.canvas.review_mark = l.mark.as_ref().filter(|m| m.start != m.end).map(|m| (m.start.clone(), m.end.clone()));
    app.canvas.scroll_to_caret = true;
}
