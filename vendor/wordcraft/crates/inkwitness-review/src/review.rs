//! The `/review/` route: open a DOCX with an InkWitness history from a local file, rebuild any
//! recorded state in WordCraft's page renderer, read-only, and step or seek through the events.
//! Seeking to an event highlights its range on the page and scrolls it into view; the
//! "Observations & notes" rail ([`crate::rail`]) shows its card. The page is a viewer: its
//! toolbar ([`crate::toolbar`]) replaces WordCraft's ribbon and holds no editing control. The
//! player dock ([`crate::player`]) plays the history at a pace a reviewer can follow, with a pin
//! for every flag ([`crate::flags`]). Teacher comments ([`crate::comments`]) live in a layer of
//! their own: they are drawn on the page as comments, but never enter the student's document or
//! its evidence.
//!
//! Nothing is uploaded: the file arrives through the app's inbox (the browser's file picker or a
//! drop) and stays in memory. The student's file is never written: the session is view-only
//! ([`wordcraft_engine::Session::set_read_only`]) and the shown document is put back if anything
//! changes it.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;
use wordcraft_doc::{Document, Pos, StoryRef};
use wordcraft_engine::{Selection, Session};
use wordcraft_inkwitness::envelope;
use wordcraft_inkwitness::replay::{Replay, ReplayError, Resolved, State};
use wordcraft_ui_egui::WordApp;

use crate::comments::{self, Draft, TeacherComment};
use crate::flags::{self, Flag};
use crate::player::{self, DockAction, Player, Timeline};
use crate::rail::RailAction;

/// Largest file the review page reads.
pub const MAX_FILE_BYTES: usize = 64 << 20;

/// The text the review page starts with.
pub const START_TEXT: &str = "InkWitness Review. Open a DOCX saved by InkWitness Write: use Open DOCX at the top right, or drop the file on this page. The file stays on this computer.";

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
    /// The student's name and the assessment, as the evidence records them (`None`: not entered).
    pub student: Option<String>,
    pub assessment: Option<String>,
    pub replay: Replay,
    pub state: State,
    pub consistency: Consistency,
    /// The last event the history replays to (all of them when it is consistent).
    pub replayable: u64,
    /// Where the writing starts ([`writing_starts`]).
    pub first: u64,
    /// The shown event's range in the shown state, if it has one.
    pub mark: Option<Resolved>,
    /// The last state the history replays to: teacher comments are anchored in it.
    pub last: State,
    /// The saved file's pictures, by key. The history records where a picture goes but not the
    /// picture, so the page takes it from the file ([`Loaded::with_pictures`]).
    pub media: BTreeMap<String, Arc<Vec<u8>>>,
}

impl Loaded {
    /// `doc` (a state of the history) with the saved file's pictures it lacks. A picture removed
    /// before the save isn't in the file, and stays a blank box.
    pub fn with_pictures(&self, doc: &mut Document) {
        for (key, bytes) in &self.media {
            doc.media.entry(key.clone()).or_insert_with(|| Arc::clone(bytes));
        }
    }
}

/// The review page's state.
#[derive(Default)]
pub struct Review {
    pub loaded: Option<Loaded>,
    /// Why the last file couldn't be opened.
    pub error: Option<String>,
    /// The status line last published ([`Review::status_changed`]).
    published: String,
    /// The loaded history's flags ([`flags::compute`]).
    pub flags: Vec<Flag>,
    /// The loaded history's time scale.
    pub timeline: Timeline,
    pub player: Player,
    /// The teacher's comments and highlights, in the order they were made. In memory only.
    pub comments: Vec<TeacherComment>,
    /// The Highlight button's colour (the last one picked).
    pub colour: wordcraft_doc::props::Highlight,
    /// The comment being written or edited.
    pub draft: Option<Draft>,
    next_id: u32,
    /// A text box in the rail had the keyboard last frame: the dock's shortcuts wait.
    pub typing: bool,
    /// The draft's text box asked for the keyboard this frame: the page must not take it back.
    pub took_focus: bool,
    /// The document last put on the page (the shown state with the teacher's comments).
    shown: Option<Document>,
    /// The comment ids the teacher's comments have on the page ([`crate::export::decorate`]).
    added: crate::export::Added,
    /// Which list the rail shows under the moment shown.
    pub tab: RailTab,
    /// A short message for the reviewer (a download started, or couldn't).
    pub notice: Option<String>,
}

/// The rail's lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RailTab {
    #[default]
    Flags,
    Comments,
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
                self.flags = flags::compute(&l);
                self.player = Player::default();
                self.timeline = Timeline::new(l.replay.events(), l.first, l.replayable, self.player.skip_idle);
                self.comments.clear();
                self.draft = None;
                self.notice = None;
                self.tab = RailTab::Flags;
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
        self.put(app);
    }

    /// Put the shown state, with the teacher's comments and highlights, on the page.
    fn put(&mut self, app: &mut WordApp) {
        if let Some(l) = &self.loaded {
            let (doc, added) = show(app, l, &self.comments);
            self.shown = Some(doc);
            self.added = added;
        }
        self.paint_highlights(app);
    }

    /// Paint the teacher's highlights that can be found in the shown state over their text. The
    /// page's document isn't changed, so the view stays where it is.
    fn paint_highlights(&self, app: &mut WordApp) {
        app.canvas.review_highlights.clear();
        let (Some(l), Some(shown)) = (&self.loaded, &self.shown) else { return };
        for c in &self.comments {
            let (Some(h), Some(anchor)) = (c.highlight, &c.anchor) else { continue };
            let Some((a, b)) = comments::locate(&l.state, anchor) else { continue };
            let (a, b) = (crate::export::to_shown(shown, &self.added, &a), crate::export::to_shown(shown, &self.added, &b));
            app.canvas.review_highlights.push((a, b, crate::toolbar::page_colour(h)));
        }
    }

    /// The Highlight button's colour (yellow until another is picked).
    pub fn highlight_colour(&self) -> wordcraft_doc::props::Highlight {
        if self.colour == wordcraft_doc::props::Highlight::None { wordcraft_doc::props::Highlight::Yellow } else { self.colour }
    }

    /// Highlight the text selected on the page in `colour`, at the moment shown. The highlight is
    /// kept with the comments, anchored in the final document; text that is gone by then can't be
    /// highlighted.
    pub fn highlight_here(&mut self, app: &mut WordApp, colour: wordcraft_doc::props::Highlight) {
        let Some(l) = &self.loaded else { return };
        self.player.pause();
        self.tab = RailTab::Comments;
        if app.session.sel.is_collapsed() {
            self.notice = Some("Select text on the page first, then press Highlight.".into());
            return;
        }
        let on_page = |p: &Pos| self.shown.as_ref().map_or_else(|| p.clone(), |d| crate::export::from_shown(d, &self.added, p));
        let (a, b) = app.session.sel.ordered();
        let placed = comments::anchor_forward(&l.state, &l.last, &on_page(&a), &on_page(&b));
        let Some((anchor, false)) = placed.filter(|(x, _)| !x.quote.trim().is_empty()) else {
            self.notice = Some("This text isn't in the final document, so it can't be highlighted. Add a comment on it instead.".into());
            return;
        };
        self.next_id += 1;
        self.comments.push(TeacherComment {
            id: self.next_id,
            text: String::new(),
            highlight: Some(colour),
            anchor: Some(anchor),
            moved: false,
            seq: (l.state.seq > 0).then_some(l.state.seq),
            from_flag: None,
            written: now_iso(),
        });
        self.colour = colour;
        self.notice = None;
        // The selection would cover the colour: the caret goes to its end.
        app.session.sel = Selection::caret(b);
        self.paint_highlights(app);
    }

    /// The first paste event, if there is one.
    pub fn first_paste(&self) -> Option<u64> {
        self.loaded.as_ref()?.replay.pastes().into_iter().find(|&s| self.loaded.as_ref().is_some_and(|l| s <= l.replayable))
    }

    /// Per frame, before the app's own logic: take new files, and put the shown document back
    /// if anything changed it (the session refuses edits; a change made around it is undone).
    pub fn logic(&mut self, app: &mut WordApp) {
        self.take_files(app);
        if self.loaded.is_some() && self.shown.as_ref().is_none_or(|d| !app.session.doc.same_content(d)) {
            self.put(app);
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

    /// The page's own chrome, drawn before the app's page: the viewer toolbar across the top, the
    /// player dock across the bottom and the "Observations & notes" rail at the right.
    pub fn chrome(&mut self, ui: &mut egui::Ui, app: &mut WordApp) {
        if !app.fonts_ready() {
            return;
        }
        let mut colour = self.highlight_colour();
        let tool = crate::toolbar::show(ui, app, self.loaded.is_some(), &mut colour);
        self.colour = colour;
        let t = wordcraft_ui_egui::theme::Tokens::get(ui.ctx());
        let a = wordcraft_ui_egui::theme::critic::Accents::get(ui.ctx());
        self.play(ui, app);
        let mut dock = None;
        if let Some(l) = &self.loaded
            && l.consistency != Consistency::Missing
            && l.replayable > 0
        {
            egui::Panel::bottom("inkwitness_player")
                .exact_size(player::HEIGHT)
                .frame(
                    egui::Frame::NONE
                        .fill(a.rail)
                        .inner_margin(egui::Margin { left: 16, right: 16, top: 6, bottom: 8 })
                        .stroke(egui::Stroke::new(1.0, t.border)),
                )
                .show(ui, |ui| dock = player::show(ui, &mut self.player, l, &self.flags, &self.timeline));
            if dock.is_none() && !self.typing {
                dock = player::shortcuts(ui, self.player.playing, l.state.seq, l.first, l.replayable, &flags::moments(&self.flags));
            }
        }
        let mut rail = None;
        egui::Panel::right("inkwitness_review")
            .default_size(crate::rail::WIDTH)
            .size_range(300.0..=560.0)
            .resizable(true)
            .frame(egui::Frame::NONE.fill(a.rail).inner_margin(egui::Margin::symmetric(14, 10)).stroke(egui::Stroke::new(1.0, t.border)))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| rail = crate::rail::show(ui, self, app));
            });
        if self.took_focus {
            // The page reclaims the keyboard after a click on the chrome (the button that opened
            // the draft); the draft's text box keeps it.
            app.canvas.want_focus = false;
            self.took_focus = false;
        }
        match tool {
            Some(crate::toolbar::ToolbarAction::Highlight(h)) => self.highlight_here(app, h),
            Some(crate::toolbar::ToolbarAction::Comment) => self.comment_here(app),
            None => {}
        }
        if let Some(d) = dock {
            self.dock(app, d);
        }
        if let Some(r) = rail {
            self.rail(app, r);
        }
    }

    /// Run playback: show the next due event, and keep frames coming while playing.
    fn play(&mut self, ui: &egui::Ui, app: &mut WordApp) {
        let Some(l) = &self.loaded else { return };
        if self.timeline.skip_idle != self.player.skip_idle {
            self.timeline = Timeline::new(l.replay.events(), l.first, l.replayable, self.player.skip_idle);
            self.player.clock = self.timeline.time_of(l.state.seq);
        }
        let now = ui.input(|i| i.time);
        let flagged = flags::moments(&self.flags);
        if let Some(s) = self.player.tick(now, &self.timeline, l.state.seq, l.replayable, &flagged) {
            self.seek(app, s);
        }
        if self.player.playing {
            ui.ctx().request_repaint();
        }
    }

    /// Go to event `seq` by hand: playback pauses there.
    pub fn go(&mut self, app: &mut WordApp, seq: u64) {
        self.player.moved(&self.timeline, seq);
        self.seek(app, seq);
    }

    fn dock(&mut self, app: &mut WordApp, d: DockAction) {
        match d {
            DockAction::Seek(s) => self.go(app, s),
            DockAction::Play => {
                let Some(l) = &self.loaded else { return };
                if let Some(s) = self.player.play(&self.timeline, l.state.seq, l.replayable) {
                    self.seek(app, s);
                }
            }
            DockAction::Pause => self.player.pause(),
            DockAction::Comment => self.comment_here(app),
        }
    }

    /// Do what the rail asked for.
    pub fn rail(&mut self, app: &mut WordApp, r: RailAction) {
        match r {
            RailAction::GoTo(s) => self.go(app, s),
            RailAction::Promote(s) => self.promote(app, s),
            RailAction::Save => self.save_draft(),
            RailAction::Discard => self.draft = None,
            RailAction::Edit(id) => {
                if let Some(c) = self.comments.iter().find(|c| c.id == id) {
                    self.draft = Some(Draft::of(c));
                }
            }
            RailAction::Delete(id) => {
                let highlight = self.comments.iter().any(|c| c.id == id && c.highlight.is_some());
                self.comments.retain(|c| c.id != id);
                if self.draft.as_ref().is_some_and(|d| d.editing == Some(id)) {
                    self.draft = None;
                }
                // A highlight is only painted: the page and the view stay as they are.
                if highlight { self.paint_highlights(app) } else { self.put(app) }
            }
            RailAction::Show(id) => {
                let Some(c) = self.comments.iter().find(|c| c.id == id) else { return };
                let to = c.seq.unwrap_or(u64::MAX);
                let last = self.loaded.as_ref().map_or(0, |l| l.replayable);
                self.go(app, to.min(last));
            }
            RailAction::Export => self.export(app),
            RailAction::Tab(t) => self.tab = t,
            RailAction::Sample => {
                let (name, bytes) = crate::sample::docx();
                self.open(app, &name, &bytes);
            }
            RailAction::Open => {
                if let Some(open) = &app.services.open_async {
                    open("document");
                }
            }
        }
    }

    /// Open a comment draft on the text selected on the page, or else on the shown event's range
    /// (a point at the caret when it has none), at the moment shown.
    pub fn comment_here(&mut self, app: &mut WordApp) {
        let Some(l) = &self.loaded else { return };
        self.player.pause();
        let on_page = |p: &Pos| self.shown.as_ref().map_or_else(|| p.clone(), |d| crate::export::from_shown(d, &self.added, p));
        let (start, end) = match (&l.mark, app.session.sel.is_collapsed()) {
            (Some(m), true) => (m.start.clone(), m.end.clone()),
            _ => {
                let (a, b) = app.session.sel.ordered();
                (on_page(&a), on_page(&b))
            }
        };
        let placed = comments::anchor_forward(&l.state, &l.last, &start, &end);
        self.draft = Some(Draft {
            editing: None,
            text: String::new(),
            moved: placed.as_ref().is_some_and(|p| p.1),
            anchor: placed.map(|p| p.0),
            seq: (l.state.seq > 0).then_some(l.state.seq),
            from_flag: None,
            focus: true,
        });
    }

    /// Promote event `seq` (a flag, or any event) to a comment draft pre-filled from the evidence,
    /// anchored on the event's text in the final document. The page goes to that moment.
    pub fn promote(&mut self, app: &mut WordApp, seq: u64) {
        self.go(app, seq);
        let Some(l) = &self.loaded else { return };
        let flag = self.flags.iter().find(|f| f.seq == seq);
        let kind = flag.map(|f| f.kind);
        let text = comments::promote_text(l, seq, kind, flag.and_then(|f| f.matched.as_deref()));
        let placed = match &l.mark {
            Some(m) if l.state.seq == seq => comments::anchor_forward(&l.state, &l.last, &m.start, &m.end),
            _ => None,
        };
        self.draft = Some(Draft {
            editing: None,
            text,
            moved: placed.as_ref().is_some_and(|p| p.1),
            anchor: placed.map(|p| p.0),
            seq: Some(seq),
            from_flag: kind.map(|k| (seq, k)),
            focus: true,
        });
    }

    /// Save the draft as a teacher comment (an edit replaces the comment's text).
    pub fn save_draft(&mut self) {
        let Some(d) = self.draft.take() else { return };
        if d.text.trim().is_empty() {
            self.draft = Some(d);
            return;
        }
        match d.editing.and_then(|id| self.comments.iter_mut().find(|c| c.id == id)) {
            Some(c) => c.text = d.text,
            None => {
                self.next_id += 1;
                self.comments.push(TeacherComment {
                    id: self.next_id,
                    text: d.text,
                    highlight: None,
                    anchor: d.anchor,
                    moved: d.moved,
                    seq: d.seq,
                    from_flag: d.from_flag,
                    written: now_iso(),
                });
                self.tab = RailTab::Comments;
            }
        }
        // Drawn on the page from the next frame ([`Review::logic`] puts the page back).
        self.shown = None;
    }

    /// Whether event `seq` has been promoted to a comment.
    pub fn promoted(&self, seq: u64) -> bool {
        self.comments.iter().any(|c| c.from_flag.is_some_and(|(s, _)| s == seq))
    }

    /// Download a review copy with the teacher's comments and highlights ([`crate::export`]).
    pub fn export(&mut self, app: &mut WordApp) {
        let Some(l) = &self.loaded else { return };
        self.notice = Some(match crate::export::review_copy(l, &self.comments) {
            Ok((name, bytes)) => match &app.services.download {
                Some(download) => match download(&name, &bytes) {
                    Ok(()) => format!("Downloading {name}. The student's file is unchanged."),
                    Err(e) => format!("The review copy couldn't be downloaded: {e}"),
                },
                None => "This page can't download files.".into(),
            },
            Err(e) => format!("The review copy couldn't be made: {e}"),
        });
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
        // No history: the document is shown as saved, with nothing to replay.
        Ok(None) => return unrecorded(name, doc),
        Err(e) => return Err(format!("{name}: {}", Consistency::Corrupt(e.to_string()).label())),
    };
    let evidence: Value = serde_json::from_slice(&env.payload).map_err(|e| format!("{name}: {}", Consistency::Corrupt(e.to_string()).label()))?;
    let entered = |v: Option<&str>| v.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let student = entered(evidence.pointer("/student/displayName").and_then(Value::as_str));
    let assessment = entered(evidence.get("assignmentId").and_then(Value::as_str));
    let replay = Replay::from_evidence(&evidence).map_err(|e| format!("{name}: {}", Consistency::Corrupt(e.to_string()).label()))?;
    let start = replay.state_at(0).map_err(|e| format!("{name}: {}", Consistency::Corrupt(e.to_string()).label()))?;
    let start_digest = start.clone().digest();
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
    let first = writing_starts(replay.events(), &start_digest, replayable);
    let last = state.clone();
    let media = doc.media;
    Ok(Loaded { name: name.to_string(), student, assessment, replay, state, consistency, replayable, first, mark: None, last, media })
}

/// Where the writing starts: the last of the events at the start of `events` (up to `upto`) that
/// open a document or leave it as it was at the start (`start_digest`). Those are the writer
/// starting up (a blank New, page settings) or the file the student began from; the timeline
/// and Play start after them, not on an empty page from before the file was opened.
pub fn writing_starts(events: &[Value], start_digest: &str, upto: u64) -> u64 {
    let mut digest = start_digest;
    let mut first = 0;
    for e in events {
        let Some(seq) = e.get("seq").and_then(Value::as_u64).filter(|s| *s <= upto) else { break };
        let d = e.get("documentStateDigest").and_then(Value::as_str).unwrap_or_default();
        if e.get("type").and_then(Value::as_str) != Some("import_document") && d != digest {
            break;
        }
        first = seq;
        digest = d;
    }
    first
}

/// A DOCX without an InkWitness history, shown as saved: "History missing", no events.
fn unrecorded(name: &str, doc: wordcraft_doc::Document) -> Result<Loaded, String> {
    let doc_last = doc.clone();
    let replay = Replay::from_evidence(&serde_json::json!({"events": []})).map_err(|e| format!("{name}: {e}"))?;
    Ok(Loaded {
        name: name.to_string(),
        student: None,
        assessment: None,
        replay,
        state: State::unrecorded(doc),
        consistency: Consistency::Missing,
        replayable: 0,
        first: 0,
        mark: None,
        last: State::unrecorded(doc_last),
        media: BTreeMap::new(),
    })
}

/// The range event `e` left in `st` (its after-anchor), checked against the text around it.
pub fn event_range(st: &State, e: &Value) -> Option<Resolved> {
    let window: Vec<String> =
        e.pointer("/ops/0/afterBlocks")?.as_array()?.iter().filter_map(|b| b.get("blockId")?.as_str().map(str::to_string)).collect();
    st.resolve(e.get("afterAnchor")?, Some(&window))
}

/// Put the loaded state on the page: the document with the teacher's comments that can be found
/// in it, the highlight, the caret at its start and the view following it. Returns the document
/// put on the page and the ids its teacher comments have.
fn show(app: &mut WordApp, l: &Loaded, notes: &[TeacherComment]) -> (Document, crate::export::Added) {
    let mut doc = l.state.doc.clone();
    l.with_pictures(&mut doc);
    let added = crate::export::decorate(&mut doc, &l.state, notes);
    let mark = l.mark.as_ref().map(|m| (crate::export::to_shown(&doc, &added, &m.start), crate::export::to_shown(&doc, &added, &m.end)));
    app.session.doc = doc.clone();
    app.session.touch();
    app.session.dirty = false;
    let at = mark.as_ref().map_or_else(|| app.session.doc.start_of(StoryRef::Body), |m| m.0.clone());
    app.session.sel = Selection::caret(app.session.doc.clamp(&at));
    app.canvas.review_mark = mark.filter(|m| m.0 != m.1);
    app.canvas.scroll_to_caret = true;
    (doc, added)
}

/// The time now, RFC 3339 (a teacher comment's date).
fn now_iso() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        crate::web::now_iso()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        chrono::DateTime::from_timestamp(i64::try_from(secs).unwrap_or(0), 0)
            .map_or_else(String::new, |d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
    }
}
