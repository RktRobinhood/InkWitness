//! InkWitness edit evidence: records every committed WordCraft edit as one event of
//! `schemas/evidence-v1.schema.json` (InkWitness repository), anchored in model units.
//!
//! [`Recorder`] is a [`CommitObserver`]: attach it to a [`Session`] and every outermost command
//! that changed the document becomes one event (no-ops and failed commands make none; Undo and
//! Redo make `undo` and `redo` events). A paste the front end raised a [`Gesture`] for carries
//! the gesture's id on exactly the event of that one command.
//!
//! The anchor rules are in `docs/adr/0001-event-anchors.md`. The digests here are **provisional
//! (`v0`)**: the canonical serialization and hashing are frozen with the DOCX envelope.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod blocks;
pub mod envelope;
pub mod evidence;
pub mod replay;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use wordcraft_doc::{Block, Document};
use wordcraft_engine::Session;
use wordcraft_engine::observe::{Commit, CommitKind, CommitObserver, Gesture};

pub use blocks::{Anchor, Op, PARA_MARK, WindowBlock, story_key};

/// Event content (the changed paragraphs' full model) is left out past this size.
pub const MAX_CONTENT_BYTES: usize = 256 * 1024;

/// Typing joins one event until a pause longer than this (ms).
pub const MERGE_PAUSE_MS: u64 = 2000;
/// A merged typing event holds at most this many key presses, so a long run without a space
/// still reads as typing and never as a large insertion or deletion.
pub const MERGE_MAX_KEYS: u64 = 32;
/// A single key press carries at most this many characters (an input method may commit a few).
const MERGE_MAX_KEY_CHARS: usize = 8;

/// Whether typing that has made `run` (all of one event's text so far) ends its event: the last
/// character is a space or punctuation mark, and a letter or digit came before it. A space typed
/// after a full stop therefore starts the next word's event instead of making one of its own.
pub fn ends_word(run: &str) -> bool {
    run.chars().next_back().is_some_and(|c| !c.is_alphanumeric()) && run.chars().any(char::is_alphanumeric)
}

/// Domain separation for the provisional hashes.
const DOMAIN_GENESIS: &str = "inkwitness:genesis:v0";
const DOMAIN_EVENT: &str = "inkwitness:event:v0";
const DOMAIN_STATE: &str = "inkwitness:doc-state:v0";
const DOMAIN_BLOCK: &str = "inkwitness:block:v0";
const DOMAIN_CONTEXT: &str = "inkwitness:anchor-context:v0";

/// Clocks and ids for the recorder: the host's ([`SystemEnv`] natively; the browser host
/// supplies its own), a fixed one in tests.
pub trait Env: Send {
    /// Wall-clock time, RFC 3339.
    fn now_iso(&mut self) -> String;
    /// Milliseconds since the session span started, never going backwards.
    fn monotonic_ms(&mut self) -> u64;
    /// A new random (version 4) UUID.
    fn uuid(&mut self) -> String;
}

/// The system clock and random ids (not on wasm, where the browser host supplies an [`Env`]).
#[cfg(not(target_arch = "wasm32"))]
pub struct SystemEnv {
    start: std::time::Instant,
    fallback: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl SystemEnv {
    pub fn new() -> Self {
        SystemEnv { start: std::time::Instant::now(), fallback: 0 }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for SystemEnv {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Env for SystemEnv {
    fn now_iso(&mut self) -> String {
        wordcraft_engine::cmd::now_iso()
    }
    fn monotonic_ms(&mut self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
    fn uuid(&mut self) -> String {
        let mut b = [0u8; 16];
        if getrandom::fill(&mut b).is_err() {
            // No randomness: still unique within this session (time and a counter).
            self.fallback = self.fallback.wrapping_add(1);
            let t = self.start.elapsed().as_nanos().to_le_bytes();
            for (i, x) in b.iter_mut().enumerate() {
                *x = t.get(i).copied().unwrap_or(0) ^ self.fallback.to_le_bytes().get(i % 8).copied().unwrap_or(0);
            }
        }
        uuid_v4(b)
    }
}

/// Format 16 random bytes as a version 4 UUID.
pub fn uuid_v4(mut b: [u8; 16]) -> String {
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex(&b);
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// Who and where the events belong to.
#[derive(Clone, Debug)]
pub struct Config {
    pub attempt_id: String,
    pub session_span_id: String,
    pub field_id: String,
    pub section_id: Option<String>,
}

/// What a typing event adds to the plain event fields.
struct Typing {
    end_ms: u64,
    keys: u64,
}

/// What a typing run is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunKind {
    /// Characters typed one after another.
    Insert,
    /// Backspaces in a row.
    Backspace,
    /// Delete key presses in a row.
    Forward,
}

/// The typing run the last event holds: what it started from, so the merged event can be
/// diffed again from there, and where the next key press has to land to join it.
struct Run {
    kind: RunKind,
    command: String,
    base: Document,
    base_ids: blocks::Ids,
    base_pos: wordcraft_doc::Pos,
    /// Where the caret is after the last key press.
    caret: wordcraft_doc::Pos,
    story: wordcraft_doc::StoryRef,
    block: String,
    /// The run's change so far, for the next key press to line up against.
    offset: usize,
    text: String,
    last_ms: u64,
    keys: u64,
    /// The last event's `priorEventDigest`.
    prior: String,
}

/// Records committed edits as evidence events. Cloning gives another handle on the same
/// record: attach one to the session, read the events through another.
#[derive(Clone)]
pub struct Recorder {
    state: Arc<Mutex<State>>,
}

struct State {
    env: Box<dyn Env>,
    config: Config,
    seq: u64,
    prior: String,
    ids: blocks::Ids,
    /// The document as of the last event: its paragraphs are what the ids name, and holding it
    /// keeps their addresses from being reused while `digests` is keyed by them.
    last: Document,
    digests: HashMap<usize, [u8; 32]>,
    events: Vec<Value>,
    /// The typing run the last event belongs to, while the next key press may still join it.
    open: Option<Run>,
    /// Whether typing joins events (always, except to make a per-letter file for a test).
    merge: bool,
    /// The event a joined key press writes again (see [`State::join`]).
    reuse: Option<Value>,
    /// Model states to replay from (`checkpoints[]`): one when the recorder attached, and one
    /// whenever a command replaced the document (an import, New, Open).
    checkpoints: Vec<Value>,
}

impl Recorder {
    /// A recorder for `doc` as it is now: its paragraphs get ids `b1`, `b2`… in document order.
    pub fn new(env: Box<dyn Env>, config: Config, doc: &Document) -> Self {
        let mut ids = blocks::Ids::default();
        ids.assign_all(doc);
        let prior = sha256_hex(DOMAIN_GENESIS, config.attempt_id.as_bytes());
        let checkpoints = vec![replay::checkpoint_json(0, doc, &ids.keyed())];
        let state = State {
            env,
            config,
            seq: 0,
            prior,
            ids,
            last: doc.clone(),
            digests: HashMap::new(),
            events: Vec::new(),
            open: None,
            merge: true,
            reuse: None,
            checkpoints,
        };
        Recorder { state: Arc::new(Mutex::new(state)) }
    }

    /// Record one event per key press, as a 1.0 writer did. For tests and fixtures: the review
    /// page reads both.
    pub fn set_merge_typing(&self, merge: bool) {
        self.lock().merge = merge;
    }

    /// Whether typing joins events (the default). The evidence says 1.1 when it does, 1.0 when
    /// it records a file the way a 1.0 writer did.
    pub fn merges_typing(&self) -> bool {
        self.lock().merge
    }

    /// A recorder attached to `session`, which now reports every committed edit to it.
    pub fn attach(session: &mut Session, env: Box<dyn Env>, config: Config) -> Self {
        let r = Recorder::new(env, config, &session.doc);
        session.set_observer(Some(Box::new(r.clone())));
        r
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The events so far, oldest first.
    pub fn events(&self) -> Vec<Value> {
        self.lock().events.clone()
    }

    /// Who and where the events belong to.
    pub fn config(&self) -> Config {
        self.lock().config.clone()
    }

    /// The checkpoints so far (the evidence's `checkpoints[]`), oldest first.
    pub fn checkpoints(&self) -> Vec<Value> {
        self.lock().checkpoints.clone()
    }

    /// Each story's paragraph ids now, in document order (`body`, `part:<id>`).
    pub fn block_ids(&self) -> BTreeMap<String, Vec<String>> {
        self.lock().ids.keyed()
    }

    /// The provisional digest of a document's state (see [`document_digest`]).
    pub fn state_digest(&self, doc: &Document) -> String {
        let mut cache = HashMap::new();
        document_digest(&mut cache, &HashMap::new(), doc)
    }
}

impl CommitObserver for Recorder {
    fn gesture_id(&mut self) -> String {
        self.lock().env.uuid()
    }

    fn on_commit(&mut self, c: &Commit<'_>) {
        self.lock().commit(c);
    }
}

/// The command family an event's origin names. `keyboard` means a typing or editing-key
/// command, not proof that a physical key was pressed.
fn keyboard_command(id: &str) -> bool {
    matches!(
        id,
        "text.insert"
            | "text.newParagraph"
            | "text.lineBreak"
            | "text.tab"
            | "text.backTab"
            | "text.insertTab"
            | "text.backspace"
            | "text.delete"
            | "text.deleteWordBack"
            | "text.deleteWordForward"
    )
}

/// The event type and origin for a change.
fn classify(command: &str, kind: CommitKind, ops: &[Op]) -> (&'static str, &'static str) {
    match kind {
        CommitKind::Undo => return ("undo", "undo"),
        CommitKind::Redo => return ("redo", "undo"),
        CommitKind::Replaced => return ("import_document", "import"),
        CommitKind::Edit => {}
    }
    if wordcraft_engine::observe::is_paste(command) {
        return ("paste", "clipboard");
    }
    if command == "edit.cut" {
        return ("cut", "editor_command");
    }
    let origin = if command.is_empty() {
        "unknown"
    } else if keyboard_command(command) {
        "keyboard"
    } else {
        "editor_command"
    };
    let text = |s: &str| s.chars().any(|c| c != PARA_MARK);
    let inserted = ops.iter().any(|o| text(&o.inserted));
    let deleted = ops.iter().any(|o| text(&o.deleted));
    let structure = ops.iter().any(|o| o.before.len() != o.after.len() || o.inserted.contains(PARA_MARK) || o.deleted.contains(PARA_MARK));
    let ty = match (inserted, deleted) {
        // Only paragraph marks came or went (a split or merge): structure, not text (W05).
        (false, false) if structure || ops.is_empty() => "structure_change",
        (false, false) => "format_change",
        (true, false) => "text_insert",
        (false, true) => "text_delete",
        (true, true) => "text_replace",
    };
    (ty, origin)
}

impl State {
    fn commit(&mut self, c: &Commit<'_>) {
        // A change that didn't come through `Session::run` (the front end edited the document
        // directly): recorded as its own event of unknown origin, so the ids stay true.
        if !self.last.same_content(c.before) {
            let before = std::mem::take(&mut self.last);
            let ops = blocks::rebind(&mut self.ids, &before, c.before, blocks::Hints::default());
            let ms = self.env.monotonic_ms();
            let json_ops = self.json_ops("", CommitKind::Edit, &ops, &before, c.before);
            self.push_event("", CommitKind::Edit, &ops, json_ops, None, c.before, ms, None);
            drop(before);
        }
        let ms = self.env.monotonic_ms();
        if c.kind == CommitKind::Replaced {
            self.ids.assign_all(c.after);
            let stories: Map<String, Value> = self.ids.stories.iter().map(|(s, ids)| (story_key(*s), json!(ids))).collect();
            let op = json!({"op": "replaceDocument", "command": c.command, "stories": stories});
            self.push_event(c.command, c.kind, &[], vec![op], None, c.after, ms, None);
            // The new document's state isn't in any op: replay starts again from here.
            let cp = replay::checkpoint_json(self.seq, c.after, &self.ids.keyed());
            self.checkpoints.push(cp);
            return;
        }
        let (start, _) = c.before_sel.ordered();
        let hints = blocks::Hints { before: Some(&start), after: Some(&c.after_sel.focus) };
        let before_ids = self.ids.clone();
        let ops = blocks::rebind(&mut self.ids, c.before, c.after, hints);
        let key = typing_key(c, &ops).filter(|_| self.merge);
        if let Some(kind) = key
            && self.join(c, kind, &start, ms)
        {
            return;
        }
        let json_ops = self.json_ops(c.command, c.kind, &ops, c.before, c.after);
        let run = key.and_then(|kind| {
            let o = ops.first()?;
            Some(Run {
                kind,
                command: c.command.to_string(),
                base: c.before.clone(),
                base_ids: before_ids,
                base_pos: start,
                caret: c.after_sel.focus.clone(),
                story: o.story,
                block: o.before.first()?.id.clone(),
                offset: o.offset,
                text: if kind == RunKind::Insert { o.inserted.clone() } else { o.deleted.clone() },
                last_ms: ms,
                keys: 1,
                prior: self.prior.clone(),
            })
        });
        let typing = run.as_ref().map(|_| Typing { end_ms: ms, keys: 1 });
        self.push_event(c.command, c.kind, &ops, json_ops, c.gesture, c.after, ms, typing);
        // A word's last character ends its event.
        self.open = run.filter(|r| !(r.kind == RunKind::Insert && ends_word(&r.text)));
    }

    /// Join this key press to the open typing run, if it continues it: the same kind of key,
    /// right at the run's caret, within the pause and key limits. The run's event is then written
    /// again as the one command the whole run amounts to. Returns whether it joined.
    fn join(&mut self, c: &Commit<'_>, kind: RunKind, start: &wordcraft_doc::Pos, ms: u64) -> bool {
        let Some(run) = self.open.as_ref() else { return false };
        if run.kind != kind
            || run.command != c.command
            || *start != run.caret
            || ms.saturating_sub(run.last_ms) > MERGE_PAUSE_MS
            || run.keys >= MERGE_MAX_KEYS
        {
            return false;
        }
        let mut ids = run.base_ids.clone();
        let hints = blocks::Hints { before: Some(&run.base_pos), after: Some(&c.after_sel.focus) };
        let ops = blocks::rebind(&mut ids, &run.base, c.after, hints);
        let [o] = ops.as_slice() else { return false };
        let fits = ids.stories == self.ids.stories
            && o.story == run.story
            && o.before.len() == 1
            && o.after.len() == 1
            && o.before.first().is_some_and(|b| b.id == run.block)
            && text_rule_rebuilds(o)
            && match kind {
                RunKind::Insert => {
                    o.deleted.is_empty() && o.offset == run.offset && o.inserted.starts_with(&run.text) && o.inserted.len() > run.text.len()
                }
                RunKind::Backspace => {
                    o.inserted.is_empty()
                        && o.offset < run.offset
                        && o.offset + o.deleted.len() == run.offset + run.text.len()
                        && o.deleted.ends_with(&run.text)
                }
                RunKind::Forward => {
                    o.inserted.is_empty() && o.offset == run.offset && o.deleted.starts_with(&run.text) && o.deleted.len() > run.text.len()
                }
            };
        if !fits {
            return false;
        }
        let json_ops = self.json_ops(c.command, c.kind, &ops, &run.base, c.after);
        if json_ops.len() != 1 {
            return false;
        }
        let (keys, prior) = (run.keys + 1, run.prior.clone());
        let (text, offset) = (if kind == RunKind::Insert { o.inserted.clone() } else { o.deleted.clone() }, o.offset);
        let Some(old) = self.events.pop() else { return false };
        self.prior = prior;
        let mut next = self.open.take();
        self.reuse = Some(old);
        self.push_event(c.command, c.kind, &ops, json_ops, None, c.after, ms, Some(Typing { end_ms: ms, keys }));
        if let Some(r) = next.as_mut() {
            r.caret = c.after_sel.focus.clone();
            r.text = text;
            r.offset = offset;
            r.last_ms = ms;
            r.keys = keys;
        }
        self.open = next.filter(|r| !(r.kind == RunKind::Insert && ends_word(&r.text)));
        true
    }

    /// An event's ops as JSON: typing carries its text only, when the plain-text rule rebuilds its
    /// paragraphs exactly; anything else carries the changed paragraphs too.
    fn json_ops(&self, command: &str, kind: CommitKind, ops: &[Op], before: &Document, after: &Document) -> Vec<Value> {
        let keyboard = classify(command, kind, ops).1 == "keyboard";
        let mut json_ops: Vec<Value> = ops.iter().map(|o| op_json(command, o, !keyboard || !text_rule_rebuilds(o), after)).collect();
        json_ops.extend(part_ops(command, before, after));
        if json_ops.is_empty() {
            // Only something outside the stories changed (styles, comments, settings…).
            json_ops.push(json!({"op": "nonStory", "command": command}));
        }
        json_ops
    }

    #[allow(clippy::too_many_arguments)]
    fn push_event(
        &mut self,
        command: &str,
        kind: CommitKind,
        ops: &[Op],
        json_ops: Vec<Value>,
        gesture: Option<&Gesture>,
        after: &Document,
        ms: u64,
        typing: Option<Typing>,
    ) {
        self.open = None;
        let (ty, origin) = classify(command, kind, ops);
        // A joined event keeps what it started with: its id, seq, and when it began.
        let old = self.reuse.take();
        let keep = |k: &str| old.as_ref().and_then(|o| o.get(k)).cloned();
        if old.is_none() {
            self.seq = self.seq.saturating_add(1);
        }
        let anchor = |a: Option<&Anchor>, state: &str| match a {
            Some(a) => json!({"blockId": a.block_id, "startOffset": a.start, "endOffset": a.end, "contextHash": a.context_hash, "state": state}),
            None => Value::Null,
        };
        let one = (ops.len() == 1).then(|| ops.first()).flatten();
        let mut e = Map::new();
        e.insert("eventId".into(), keep("eventId").unwrap_or_else(|| json!(self.env.uuid())));
        e.insert("attemptId".into(), json!(self.config.attempt_id));
        e.insert("sessionSpanId".into(), json!(self.config.session_span_id));
        e.insert("seq".into(), json!(self.seq));
        if let Some(s) = &self.config.section_id {
            e.insert("sectionId".into(), json!(s));
        }
        e.insert("fieldId".into(), json!(self.config.field_id));
        e.insert("type".into(), json!(ty));
        e.insert("origin".into(), json!(origin));
        e.insert("atLocalIso".into(), keep("atLocalIso").unwrap_or_else(|| json!(self.env.now_iso())));
        e.insert("elapsedMonotonicMs".into(), keep("elapsedMonotonicMs").unwrap_or_else(|| json!(ms)));
        if let Some(t) = typing {
            e.insert("endMonotonicMs".into(), json!(t.end_ms));
            e.insert("keyCount".into(), json!(t.keys));
        }
        let gesture_id = gesture.and_then(|g| g.id.clone());
        e.insert("gestureId".into(), json!(gesture_id));
        let first = ops.first();
        e.insert("beforeAnchor".into(), anchor(first.and_then(|o| o.before_anchor.as_ref()), "before"));
        e.insert("afterAnchor".into(), anchor(first.and_then(|o| o.after_anchor.as_ref()), "after"));
        e.insert("ops".into(), Value::Array(json_ops));
        e.insert("insertedText".into(), json!(one.map(|o| o.inserted.clone())));
        e.insert("deletedText".into(), json!(one.map(|o| o.deleted.clone())));
        if ty == "paste" {
            if let Some(types) = gesture.and_then(|g| g.clipboard_types.clone()) {
                e.insert("clipboardTypes".into(), json!(types));
            }
            // A browser can't say which application the text came from.
            e.insert("sourceHint".into(), Value::Null);
            e.insert("sourceStatus".into(), json!("unknown"));
            e.insert("pasteGestureId".into(), json!(gesture_id));
        }
        e.insert("documentStateDigest".into(), json!(self.state_digest(after)));
        e.insert("priorEventDigest".into(), json!(self.prior));
        let digest = sha256_hex(DOMAIN_EVENT, canonical_json(&Value::Object(e.clone())).as_bytes());
        e.insert("eventDigest".into(), json!(digest));
        self.prior = digest;
        self.events.push(Value::Object(e));
        self.last = after.clone();
    }

    /// The state digest of `doc`, reusing the digests of paragraphs and tables it shares with
    /// the last document (held in `last`, so their addresses can't have been reused).
    fn state_digest(&mut self, doc: &Document) -> String {
        let old = std::mem::take(&mut self.digests);
        let mut cache = HashMap::new();
        let d = document_digest(&mut cache, &old, doc);
        self.digests = cache;
        d
    }
}

/// Whether the commit is a single key press of typing, a Backspace or a Delete, in one paragraph
/// and with nothing selected: what a typing event is made of. Paste, selection deletes, undo and
/// every other command are never one of these.
fn typing_key(c: &Commit<'_>, ops: &[Op]) -> Option<RunKind> {
    let [o] = ops else { return None };
    if c.kind != CommitKind::Edit || c.gesture.is_some() || !c.before_sel.is_collapsed() {
        return None;
    }
    if !(o.siblings && o.before.len() == 1 && o.after.len() == 1 && o.before.first()?.id == o.after.first()?.id && text_rule_rebuilds(o)) {
        return None;
    }
    let plain = |t: &str| !t.is_empty() && t.chars().count() <= MERGE_MAX_KEY_CHARS && !t.contains(PARA_MARK);
    match c.command {
        "text.insert" if o.deleted.is_empty() && plain(&o.inserted) => Some(RunKind::Insert),
        "text.backspace" if o.inserted.is_empty() && plain(&o.deleted) => Some(RunKind::Backspace),
        "text.delete" if o.inserted.is_empty() && plain(&o.deleted) => Some(RunKind::Forward),
        _ => None,
    }
}

/// Whether replay's plain-text rule rebuilds an op's paragraphs exactly from its text change.
fn text_rule_rebuilds(o: &Op) -> bool {
    let before: Vec<&wordcraft_doc::Paragraph> = o.before_blocks.iter().filter_map(|b| b.as_para()).collect();
    let after: Vec<&wordcraft_doc::Paragraph> = o.after_blocks.iter().filter_map(|b| b.as_para()).collect();
    if !o.siblings || before.len() != o.before_blocks.len() || after.len() != o.after_blocks.len() {
        return false;
    }
    replay::apply_text(&before, o.offset, &o.deleted, &o.inserted).is_some_and(|rebuilt| rebuilt.iter().eq(after.iter().copied()))
}

/// `setPart` ops for the document parts outside the stories that a replay needs to lay the
/// document out as it was ([`replay::RECORDED_PARTS`]: page setup, styles, numbering), one per
/// part that changed.
fn part_ops(command: &str, before: &Document, after: &Document) -> Vec<Value> {
    let mut out = Vec::new();
    let mut push = |part: &str, changed: bool, value: serde_json::Result<Value>| {
        if !changed {
            return;
        }
        let mut m = Map::new();
        m.insert("op".into(), json!("setPart"));
        m.insert("command".into(), json!(command));
        m.insert("part".into(), json!(part));
        match value {
            Ok(v) if serde_json::to_string(&v).map_or(usize::MAX, |s| s.len()) <= MAX_CONTENT_BYTES => {
                m.insert("value".into(), v);
            }
            _ => {
                m.insert("valueOmitted".into(), json!(format!("larger than {MAX_CONTENT_BYTES} bytes")));
            }
        }
        out.push(Value::Object(m));
    };
    push("lastSection", before.last_section != after.last_section, serde_json::to_value(&after.last_section));
    push("styles", before.styles != after.styles, serde_json::to_value(&after.styles));
    push("numbering", before.numbering != after.numbering, serde_json::to_value(&after.numbering));
    out
}

fn op_json(command: &str, o: &Op, with_content: bool, after: &Document) -> Value {
    let win = |w: &[WindowBlock]| -> Value { w.iter().map(|b| json!({"blockId": b.id, "len": b.len})).collect() };
    let mut m = Map::new();
    m.insert("op".into(), json!("replace"));
    m.insert("command".into(), json!(command));
    m.insert("story".into(), json!(story_key(o.story)));
    m.insert("beforeBlocks".into(), win(&o.before));
    m.insert("afterBlocks".into(), win(&o.after));
    m.insert("offset".into(), json!(o.offset));
    m.insert("deleted".into(), json!(o.deleted));
    m.insert("inserted".into(), json!(o.inserted));
    if with_content {
        let content: Vec<Value> = o.after_blocks.iter().map(|b| serde_json::to_value(&**b).unwrap_or(Value::Null)).collect();
        let size = serde_json::to_string(&content).map_or(usize::MAX, |s| s.len());
        if size <= MAX_CONTENT_BYTES {
            m.insert("content".into(), Value::Array(content));
        } else {
            m.insert("contentOmitted".into(), json!(format!("larger than {MAX_CONTENT_BYTES} bytes")));
        }
    }
    // A table came or went in the window: its paragraphs alone can't rebuild the story.
    if !o.siblings
        && let Some((_, bl)) = blocks::stories(after).into_iter().find(|(s, _)| *s == o.story)
    {
        let story: Vec<Value> = bl.iter().map(|b| serde_json::to_value(&**b).unwrap_or(Value::Null)).collect();
        if serde_json::to_string(&story).map_or(usize::MAX, |s| s.len()) <= 4 * MAX_CONTENT_BYTES {
            m.insert("storyBlocks".into(), Value::Array(story));
        } else {
            m.insert("storyBlocksOmitted".into(), json!(format!("larger than {} bytes", 4 * MAX_CONTENT_BYTES)));
        }
    }
    Value::Object(m)
}

/// Provisional (`v0`) digest of a document's story content: every story's blocks (paragraph
/// text, runs, paragraph properties, inline objects, tables) in order. Styles, numbering,
/// comments, media and settings are not part of it yet.
fn document_digest(cache: &mut HashMap<usize, [u8; 32]>, old: &HashMap<usize, [u8; 32]>, doc: &Document) -> String {
    let mut h = Sha256::new();
    h.update(DOMAIN_STATE.as_bytes());
    h.update([0]);
    for (s, bl) in blocks::stories(doc) {
        h.update(story_key(s).as_bytes());
        h.update([0]);
        h.update((bl.len() as u64).to_le_bytes());
        for b in bl {
            let key = Arc::as_ptr(b) as usize;
            let d = match old.get(&key).or_else(|| cache.get(&key)) {
                Some(d) => *d,
                None => block_digest(b),
            };
            cache.insert(key, d);
            h.update(d);
        }
    }
    hex(&h.finalize())
}

fn block_digest(b: &Block) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(DOMAIN_BLOCK.as_bytes());
    h.update([0]);
    h.update(serde_json::to_vec(b).unwrap_or_default());
    let mut out = [0u8; 32];
    out.copy_from_slice(&h.finalize());
    out
}

/// Hash of the text around an anchor (up to 16 characters each side), so a reader can tell the
/// anchor still names the same place.
pub fn context_hash(before: &str, after: &str) -> String {
    let mut data = Vec::with_capacity(before.len() + after.len() + 1);
    data.extend_from_slice(before.as_bytes());
    data.push(0);
    data.extend_from_slice(after.as_bytes());
    sha256_hex(DOMAIN_CONTEXT, &data)
}

/// SHA-256 of `domain`, a zero byte and `data`, as lowercase hex.
pub fn sha256_hex(domain: &str, data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(domain.as_bytes());
    h.update([0]);
    h.update(data);
    hex(&h.finalize())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// JSON with object keys sorted and no whitespace (provisional; RFC 8785 JCS is the target).
pub fn canonical_json(v: &Value) -> String {
    let mut out = String::new();
    write_canonical(v, &mut out);
    out
}

fn write_canonical(v: &Value, out: &mut String) {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(k.clone()).to_string());
                out.push(':');
                if let Some(x) = m.get(k) {
                    write_canonical(x, out);
                }
            }
            out.push('}');
        }
        Value::Array(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(x, out);
            }
            out.push(']');
        }
        _ => out.push_str(&v.to_string()),
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_replay;
#[cfg(test)]
mod tests_sessions;
