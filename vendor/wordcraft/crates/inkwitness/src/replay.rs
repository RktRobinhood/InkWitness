//! Replay (backlog G0-6): rebuild the WordCraft document as it was after any recorded event.
//!
//! A replay starts from the latest checkpoint at or before the event asked for (the model state
//! and each story's block ids, written by the [`Recorder`](crate::Recorder) when it attached and
//! whenever the document was replaced) and applies the recorded events one by one. Each event's
//! ops name the paragraphs they replace by block id, so nothing is found by searching for text,
//! and every step is checked:
//!
//! - an op's window must be exactly the blocks it names, with the lengths and the deleted text it
//!   recorded;
//! - after each event the state digest must equal the event's `documentStateDigest`.
//!
//! A step that fails either check stops the replay with a [`ReplayError`]. Nothing is guessed.
//!
//! Anchors resolve to WordCraft [`Pos`]itions in the rebuilt state ([`State::resolve`]). Page
//! numbers and coordinates come from laying that state out, never from the evidence.

use std::collections::{BTreeMap, HashMap};
use std::io::Write as _;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::{Value, json};
use wordcraft_doc::{Block, Blocks, Document, Paragraph, Path, Pos, StoryRef};

use crate::blocks::{self, PARA_MARK, story_key};

/// Why a replay stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// The evidence is not a replayable event list (missing fields, events out of order).
    Evidence(String),
    /// No checkpoint at or before this event, or none for the document this import put in place.
    NoCheckpoint(u64),
    /// A checkpoint can't be read, or its state doesn't match its digest.
    Checkpoint(String),
    /// An op of this event doesn't fit the state it should apply to.
    Op { seq: u64, why: String },
    /// After this event the rebuilt state doesn't match the event's `documentStateDigest`.
    Digest { seq: u64 },
    /// This event changed something replay can't rebuild yet.
    Unsupported { seq: u64, what: String },
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::Evidence(w) => write!(f, "the evidence can't be replayed: {w}"),
            ReplayError::NoCheckpoint(seq) => write!(f, "no checkpoint to replay event {seq} from"),
            ReplayError::Checkpoint(w) => write!(f, "checkpoint: {w}"),
            ReplayError::Op { seq, why } => write!(f, "event {seq} doesn't apply: {why}"),
            ReplayError::Digest { seq } => write!(f, "after event {seq} the rebuilt document differs from the recorded one"),
            ReplayError::Unsupported { seq, what } => write!(f, "event {seq}: replay can't rebuild {what} yet"),
        }
    }
}

impl std::error::Error for ReplayError {}

/// The document parts outside the stories that events record when they change (op `setPart`).
pub const RECORDED_PARTS: &[&str] = &["lastSection", "styles", "numbering"];

/// Largest checkpoint state, uncompressed.
pub const MAX_CHECKPOINT_BYTES: usize = 64 * 1024 * 1024;

/// A checkpoint as the evidence schema writes it (`checkpoints[]`): the state after event
/// `afterSeq`, its digest, and the gzipped JSON `{"blockIds": {story: [ids]}, "document": …}`.
pub fn checkpoint_json(after_seq: u64, doc: &Document, ids: &BTreeMap<String, Vec<String>>) -> Value {
    // The evidence part itself is never part of the state it describes.
    let mut doc = doc.clone();
    let _ = wordcraft_docx::remove_custom_xml_part(&mut doc, crate::envelope::NAMESPACE);
    let digest = crate::document_digest(&mut HashMap::new(), &HashMap::new(), &doc);
    let state = json!({"blockIds": ids, "document": doc});
    let raw = serde_json::to_vec(&state).unwrap_or_default();
    let mut gz = GzEncoder::new(Vec::new(), Compression::best());
    let compressed = match gz.write_all(&raw) {
        Ok(()) => gz.finish().unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    json!({"afterSeq": after_seq, "canonicalDocumentStateDigest": digest, "compressedStateBase64": STANDARD.encode(compressed)})
}

/// The document as of one event, with its block ids.
#[derive(Clone, Debug)]
pub struct State {
    /// The last event applied (0: the first checkpoint, before any event).
    pub seq: u64,
    pub doc: Document,
    ids: BTreeMap<StoryRef, Vec<String>>,
    /// Paragraph digests of `doc`, by address (see [`crate::document_digest`]).
    digests: HashMap<usize, [u8; 32]>,
}

/// An anchor resolved in a state: the range it names, and whether the text around it is the
/// text the recorder hashed (`contextHash`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub start: Pos,
    pub end: Pos,
    pub context_matches: bool,
}

impl State {
    /// A document with no recorded history (it has no block ids), shown as it is: what a reviewer
    /// sees for a file whose history is missing. Nothing replays from it.
    pub fn unrecorded(doc: Document) -> State {
        State { seq: 0, doc, ids: BTreeMap::new(), digests: HashMap::new() }
    }

    /// Each story's paragraph ids, in document order.
    pub fn block_ids(&self) -> BTreeMap<String, Vec<String>> {
        self.ids.iter().map(|(s, ids)| (story_key(*s), ids.clone())).collect()
    }

    /// The state digest (as events record it in `documentStateDigest`).
    pub fn digest(&mut self) -> String {
        let old = std::mem::take(&mut self.digests);
        let mut cache = HashMap::new();
        let d = crate::document_digest(&mut cache, &old, &self.doc);
        self.digests = cache;
        d
    }

    /// Where block `id` is: its story and its index among the story's paragraphs.
    fn find(&self, id: &str) -> Option<(StoryRef, usize)> {
        self.ids.iter().find_map(|(s, ids)| ids.iter().position(|x| x == id).map(|k| (*s, k)))
    }

    /// The range an anchor (`{blockId, startOffset, endOffset, contextHash}`) names here. A range
    /// running past its block continues through the following paragraphs, each paragraph mark
    /// counting as [`PARA_MARK`]. `window` is the op's block list the anchor was taken in (its
    /// `afterBlocks` or `beforeBlocks`), which the context hash covers; without it the context
    /// is not checked.
    pub fn resolve(&self, anchor: &Value, window: Option<&[String]>) -> Option<Resolved> {
        let id = anchor.get("blockId")?.as_str()?;
        let start = usize::try_from(anchor.get("startOffset")?.as_u64()?).ok()?;
        let end = usize::try_from(anchor.get("endOffset")?.as_u64()?).ok()?;
        let (story, k) = self.find(id)?;
        let paras = blocks::paragraphs(story_blocks(&self.doc, story)?);
        let texts: Vec<&str> = paras.iter().map(|(_, b)| b.as_para().map_or("", |p| p.text.as_str())).collect();
        let locate = |mut off: usize| -> Option<Pos> {
            let mut j = k;
            loop {
                let len = texts.get(j)?.len();
                if off <= len {
                    let path = paras.get(j)?.0.clone();
                    return texts.get(j)?.is_char_boundary(off).then(|| Pos::new(story, Path(path), off));
                }
                off -= len + PARA_MARK.len_utf8();
                j += 1;
            }
        };
        let (a, b) = (locate(start)?, locate(end.max(start))?);
        let context_matches = match (window, anchor.get("contextHash").and_then(Value::as_str)) {
            (Some(w), Some(hash)) => self.context_hash(story, w, id, start, end).as_deref() == Some(hash),
            _ => false,
        };
        Some(Resolved { start: a, end: b, context_matches })
    }

    /// The context hash of `start..end` from block `id`, in the flow of the window `w`.
    fn context_hash(&self, story: StoryRef, w: &[String], id: &str, start: usize, end: usize) -> Option<String> {
        let ids = self.ids.get(&story)?;
        let first = ids.iter().position(|x| Some(x) == w.first())?;
        let paras = blocks::paragraphs(story_blocks(&self.doc, story)?);
        let mut flow = String::new();
        let mut base = None;
        for (i, wid) in w.iter().enumerate() {
            if ids.get(first + i) != Some(wid) {
                return None;
            }
            if i > 0 {
                flow.push(PARA_MARK);
            }
            if wid == id {
                base = Some(flow.len());
            }
            flow.push_str(paras.get(first + i)?.1.as_para().map_or("", |p| p.text.as_str()));
        }
        let base = base?;
        let (s, e) = (base + start, base + end.max(start));
        let head: String = {
            let mut c: Vec<char> = flow.get(..s)?.chars().rev().take(16).collect();
            c.reverse();
            c.into_iter().collect()
        };
        let tail: String = flow.get(e..)?.chars().take(16).collect();
        Some(crate::context_hash(&head, &tail))
    }

    /// The text from `a` to `b` in this state (paragraph marks as [`PARA_MARK`]).
    pub fn text(&self, a: &Pos, b: &Pos) -> String {
        let Some(bl) = story_blocks(&self.doc, a.story) else { return String::new() };
        let mut out = String::new();
        let mut on = false;
        for (path, blk) in blocks::paragraphs(bl) {
            let t = blk.as_para().map_or("", |p| p.text.as_str());
            let here_a = path == a.path.0;
            let here_b = path == b.path.0;
            if here_a {
                on = true;
            } else if on {
                out.push(PARA_MARK);
            }
            if on {
                let s = if here_a { a.off.min(t.len()) } else { 0 };
                let e = if here_b { b.off.min(t.len()) } else { t.len() };
                out.push_str(t.get(s..e.max(s)).unwrap_or_default());
            }
            if here_b {
                break;
            }
        }
        out
    }

    /// Story `s`'s paragraphs in document order: each one's block id, position path and text.
    /// What a teacher comment's anchor is found in (FAL-519).
    pub fn paragraph_texts(&self, s: StoryRef) -> Vec<(String, Vec<u32>, String)> {
        let (Some(bl), Some(ids)) = (story_blocks(&self.doc, s), self.ids.get(&s)) else { return Vec::new() };
        blocks::paragraphs(bl)
            .into_iter()
            .zip(ids)
            .map(|((path, b), id)| (id.clone(), path, b.as_para().map_or(String::new(), |p| p.text.clone())))
            .collect()
    }

    /// The block id of the paragraph `p` is in.
    pub fn block_at(&self, p: &Pos) -> Option<String> {
        let bl = story_blocks(&self.doc, p.story)?;
        let k = blocks::paragraphs(bl).iter().position(|(path, _)| *path == p.path.0)?;
        self.ids.get(&p.story)?.get(k).cloned()
    }
}

fn story_blocks(doc: &Document, s: StoryRef) -> Option<&Blocks> {
    match s {
        StoryRef::Body => Some(&doc.body),
        StoryRef::Part(n) => doc.parts.get(&n).map(|p| &p.blocks),
    }
}

fn story_blocks_mut(doc: &mut Document, s: StoryRef) -> Option<&mut Blocks> {
    match s {
        StoryRef::Body => Some(&mut doc.body),
        StoryRef::Part(n) => doc.parts.get_mut(&n).map(|p| &mut p.blocks),
    }
}

fn parse_story(key: &str) -> Option<StoryRef> {
    if key == "body" {
        return Some(StoryRef::Body);
    }
    key.strip_prefix("part:")?.parse().ok().map(StoryRef::Part)
}

/// The container of the paragraph at `path` (its parent's block list), to splice.
fn container_mut<'a>(blocks: &'a mut Blocks, path: &[u32]) -> Option<&'a mut Blocks> {
    let (_, parents) = path.split_last()?;
    let mut cur = blocks;
    let mut rest = parents;
    while let [b, r, c, tail @ ..] = rest {
        let blk = cur.get_mut(usize::try_from(*b).ok()?)?;
        let Block::Table(t) = Arc::make_mut(blk) else { return None };
        cur = &mut t.rows.get_mut(usize::try_from(*r).ok()?)?.cells.get_mut(usize::try_from(*c).ok()?)?.blocks;
        rest = tail;
    }
    rest.is_empty().then_some(cur)
}

/// The plain-text rule: apply a text change (`deleted` at `offset` in the window's flow becomes
/// `inserted`; paragraph marks split and merge paragraphs) to a window of paragraphs. Inserted
/// text takes the formatting in effect where it goes; a merge keeps the first paragraph's
/// properties; a split gives the new paragraphs the split one's properties.
///
/// The recorder leaves an op's `content` out only when this rule rebuilds exactly the
/// paragraphs the command made, so replay is exact either way.
pub(crate) fn apply_text(window: &[&Paragraph], offset: usize, deleted: &str, inserted: &str) -> Option<Vec<Paragraph>> {
    let mut paras: Vec<Paragraph> = window.iter().map(|p| (*p).clone()).collect();
    let mut starts = Vec::with_capacity(paras.len());
    let mut flow = String::new();
    for (i, p) in paras.iter().enumerate() {
        if i > 0 {
            flow.push(PARA_MARK);
        }
        starts.push(flow.len());
        flow.push_str(&p.text);
    }
    if flow.get(offset..offset.checked_add(deleted.len())?)? != deleted {
        return None;
    }
    let locate = |x: usize| -> Option<(usize, usize)> {
        let k = starts.iter().rposition(|&s| s <= x)?;
        Some((k, x - starts.get(k)?))
    };
    let (sk, so) = locate(offset)?;
    let (ek, eo) = locate(offset.checked_add(deleted.len())?)?;
    if sk == ek {
        paras.get_mut(sk)?.delete(so, eo).ok()?;
    } else {
        let len = paras.get(sk)?.text.len();
        paras.get_mut(sk)?.delete(so, len).ok()?;
        let mut tail = paras.get(ek)?.clone();
        tail.delete(0, eo).ok()?;
        paras.drain(sk + 1..=ek);
        paras.get_mut(sk)?.append(tail);
    }
    let mut pieces = inserted.split(PARA_MARK);
    let first = pieces.next().unwrap_or_default();
    let rest: Vec<&str> = pieces.collect();
    let props = paras.get(sk)?.props_at(so).clone();
    let p = paras.get_mut(sk)?;
    if p.insert_text(so, first, &props).ok()? != first.len() {
        return None;
    }
    if let Some((last, middle)) = rest.split_last() {
        let at = so + first.len();
        let mut tail = paras.get_mut(sk)?.split_off(at).ok()?;
        let para_props = paras.get(sk)?.props.clone();
        let mut new = Vec::with_capacity(middle.len() + 1);
        for m in middle {
            let mut q = Paragraph::with_text(m, props.clone());
            q.props = para_props.clone();
            q.mark = props.clone();
            new.push(q);
        }
        if tail.insert_text(0, last, &props).ok()? != last.len() {
            return None;
        }
        new.push(tail);
        let at = sk + 1;
        paras.splice(at..at, new);
    }
    Some(paras)
}

/// A recorded session, ready to rebuild any state from.
#[derive(Clone, Debug)]
pub struct Replay {
    events: Vec<Value>,
    checkpoints: Vec<(u64, String, String)>,
    final_digest: Option<String>,
}

impl Replay {
    /// Read the evidence JSON (`schemas/evidence-v1.schema.json`): its events must be numbered
    /// 1, 2, 3… in order, and it needs at least the checkpoint before its first event.
    pub fn from_evidence(evidence: &Value) -> Result<Replay, ReplayError> {
        let events = evidence.get("events").and_then(Value::as_array).ok_or_else(|| ReplayError::Evidence("no events array".into()))?;
        for (i, e) in events.iter().enumerate() {
            let seq = e.get("seq").and_then(Value::as_u64);
            if seq != u64::try_from(i + 1).ok() {
                return Err(ReplayError::Evidence(format!("event {} has seq {seq:?}", i + 1)));
            }
            if !e.get("ops").is_some_and(Value::is_array) {
                return Err(ReplayError::Evidence(format!("event {} has no ops", i + 1)));
            }
        }
        let mut checkpoints = Vec::new();
        for c in evidence.get("checkpoints").and_then(Value::as_array).into_iter().flatten() {
            let seq = c.get("afterSeq").and_then(Value::as_u64).ok_or_else(|| ReplayError::Checkpoint("no afterSeq".into()))?;
            let digest = c.get("canonicalDocumentStateDigest").and_then(Value::as_str).unwrap_or_default().to_string();
            let state = c.get("compressedStateBase64").and_then(Value::as_str).unwrap_or_default().to_string();
            checkpoints.push((seq, digest, state));
        }
        checkpoints.sort_by_key(|c| c.0);
        let final_digest = evidence.pointer("/export/recordedStateDigest").and_then(Value::as_str).map(str::to_string);
        Ok(Replay { events: events.clone(), checkpoints, final_digest })
    }

    /// The events, oldest first (`seq` is the index plus one).
    pub fn events(&self) -> &[Value] {
        &self.events
    }

    /// The event numbered `seq`.
    pub fn event(&self, seq: u64) -> Option<&Value> {
        self.events.get(usize::try_from(seq.checked_sub(1)?).ok()?)
    }

    /// The last event's number.
    pub fn last_seq(&self) -> u64 {
        u64::try_from(self.events.len()).unwrap_or(u64::MAX)
    }

    /// The `seq` of every `paste` event.
    pub fn pastes(&self) -> Vec<u64> {
        self.events.iter().filter(|e| e.get("type").and_then(Value::as_str) == Some("paste")).filter_map(|e| e.get("seq")?.as_u64()).collect()
    }

    /// The state the export says it recorded last (`export.recordedStateDigest`).
    pub fn final_digest(&self) -> Option<&str> {
        self.final_digest.as_deref()
    }

    fn load_checkpoint(&self, i: usize) -> Result<State, ReplayError> {
        let (seq, digest, b64) = self.checkpoints.get(i).ok_or(ReplayError::NoCheckpoint(0))?;
        let gz = STANDARD.decode(b64).map_err(|_| ReplayError::Checkpoint(format!("after {seq}: not base64")))?;
        let raw = crate::envelope::gunzip_bounded(&gz, MAX_CHECKPOINT_BYTES).map_err(|e| ReplayError::Checkpoint(format!("after {seq}: {e}")))?;
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Saved {
            block_ids: BTreeMap<String, Vec<String>>,
            document: Document,
        }
        let saved: Saved = serde_json::from_slice(&raw).map_err(|e| ReplayError::Checkpoint(format!("after {seq}: {e}")))?;
        let mut ids = BTreeMap::new();
        for (k, v) in saved.block_ids {
            let s = parse_story(&k).ok_or_else(|| ReplayError::Checkpoint(format!("after {seq}: story {k:?}")))?;
            ids.insert(s, v);
        }
        let mut st = State { seq: *seq, doc: saved.document, ids, digests: HashMap::new() };
        for (s, bl) in blocks::stories(&st.doc) {
            if st.ids.get(&s).map_or(0, Vec::len) != blocks::paragraphs(bl).len() {
                return Err(ReplayError::Checkpoint(format!("after {seq}: {} block ids don't fit its paragraphs", story_key(s))));
            }
        }
        if st.digest() != *digest {
            return Err(ReplayError::Checkpoint(format!("after {seq}: the state doesn't match its digest")));
        }
        Ok(st)
    }

    /// The state after event `seq` (0: before the first event).
    pub fn state_at(&self, seq: u64) -> Result<State, ReplayError> {
        let i = self.checkpoints.iter().rposition(|c| c.0 <= seq).ok_or(ReplayError::NoCheckpoint(seq))?;
        let mut st = self.load_checkpoint(i)?;
        self.advance(&mut st, seq)?;
        Ok(st)
    }

    /// Bring `st` forward to the state after event `seq` (a later one than its own).
    pub fn advance(&self, st: &mut State, seq: u64) -> Result<(), ReplayError> {
        while st.seq < seq {
            let next = st.seq + 1;
            let e = self.event(next).ok_or_else(|| ReplayError::Evidence(format!("no event {next}")))?;
            self.apply(st, next, e)?;
        }
        Ok(())
    }

    /// Replay every event, checking each one, and the result against the export's digest.
    pub fn verify_all(&self) -> Result<State, ReplayError> {
        let mut st = self.state_at(self.last_seq())?;
        if let Some(d) = &self.final_digest
            && st.digest() != *d
        {
            return Err(ReplayError::Digest { seq: st.seq });
        }
        Ok(st)
    }

    fn apply(&self, st: &mut State, seq: u64, e: &Value) -> Result<(), ReplayError> {
        let ops = e.get("ops").and_then(Value::as_array).ok_or_else(|| ReplayError::Evidence(format!("event {seq} has no ops")))?;
        // Holding the state before keeps its paragraphs' addresses from being reused while the
        // digest cache is keyed by them.
        let before = st.doc.clone();
        for op in ops {
            match op.get("op").and_then(Value::as_str) {
                Some("replace") => apply_replace(st, seq, op)?,
                Some("setPart") => apply_part(st, seq, op)?,
                Some("nonStory") => {}
                Some("replaceDocument") => {
                    let i = self.checkpoints.iter().position(|c| c.0 == seq).ok_or(ReplayError::NoCheckpoint(seq))?;
                    let loaded = self.load_checkpoint(i)?;
                    let stories: BTreeMap<String, Vec<String>> = op
                        .get("stories")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .ok_or_else(|| ReplayError::Op { seq, why: "replaceDocument without stories".into() })?;
                    if stories != loaded.block_ids() {
                        return Err(ReplayError::Op { seq, why: "the checkpoint's block ids differ from the import's".into() });
                    }
                    *st = loaded;
                }
                other => return Err(ReplayError::Unsupported { seq, what: format!("op {other:?}") }),
            }
        }
        st.seq = seq;
        if let Some(want) = e.get("documentStateDigest").and_then(Value::as_str)
            && st.digest() != want
        {
            return Err(ReplayError::Digest { seq });
        }
        drop(before);
        Ok(())
    }
}

fn window_ids(op: &Value, key: &str) -> Option<Vec<(String, usize)>> {
    op.get(key)?.as_array()?.iter().map(|b| Some((b.get("blockId")?.as_str()?.to_string(), usize::try_from(b.get("len")?.as_u64()?).ok()?))).collect()
}

fn apply_part(st: &mut State, seq: u64, op: &Value) -> Result<(), ReplayError> {
    let part = op.get("part").and_then(Value::as_str).unwrap_or_default();
    let Some(value) = op.get("value") else {
        return Err(ReplayError::Unsupported { seq, what: format!("{part} (too large to record)") });
    };
    let bad = |e: serde_json::Error| ReplayError::Op { seq, why: format!("{part}: {e}") };
    match part {
        "lastSection" => st.doc.last_section = serde_json::from_value(value.clone()).map_err(bad)?,
        "styles" => st.doc.styles = serde_json::from_value(value.clone()).map_err(bad)?,
        "numbering" => st.doc.numbering = serde_json::from_value(value.clone()).map_err(bad)?,
        other => return Err(ReplayError::Unsupported { seq, what: format!("part {other:?}") }),
    }
    Ok(())
}

fn apply_replace(st: &mut State, seq: u64, op: &Value) -> Result<(), ReplayError> {
    let fail = |why: &str| ReplayError::Op { seq, why: why.to_string() };
    let story_key_s = op.get("story").and_then(Value::as_str).ok_or_else(|| fail("no story"))?;
    let story = parse_story(story_key_s).ok_or_else(|| fail("unknown story"))?;
    let before = window_ids(op, "beforeBlocks").ok_or_else(|| fail("no beforeBlocks"))?;
    let after = window_ids(op, "afterBlocks").ok_or_else(|| fail("no afterBlocks"))?;
    let offset = op.get("offset").and_then(Value::as_u64).and_then(|o| usize::try_from(o).ok()).ok_or_else(|| fail("no offset"))?;
    let deleted = op.get("deleted").and_then(Value::as_str).ok_or_else(|| fail("no deleted text"))?;
    let inserted = op.get("inserted").and_then(Value::as_str).ok_or_else(|| fail("no inserted text"))?;
    if before.is_empty() {
        return Err(ReplayError::Unsupported { seq, what: format!("a new story ({story_key_s})") });
    }
    let ids = st.ids.get(&story).ok_or_else(|| ReplayError::Unsupported { seq, what: format!("story {story_key_s}") })?;
    let first = ids.iter().position(|x| Some(x) == before.first().map(|b| &b.0)).ok_or_else(|| fail("its first block isn't in the story"))?;
    if ids.get(first..first + before.len()).is_none_or(|w| !w.iter().zip(&before).all(|(a, (b, _))| a == b)) {
        return Err(fail("its blocks aren't consecutive in the story"));
    }
    let blocks_now = story_blocks(&st.doc, story).ok_or_else(|| fail("no such story"))?;
    let paras = blocks::paragraphs(blocks_now);
    let window: Vec<blocks::Para<'_>> = paras.get(first..first + before.len()).ok_or_else(|| fail("the story is shorter"))?.to_vec();
    let window_paras: Vec<&Paragraph> = window.iter().filter_map(|(_, b)| b.as_para()).collect();
    if window_paras.len() != before.len() || window_paras.iter().zip(&before).any(|(p, (_, len))| p.text.len() != *len) {
        return Err(fail("its blocks' lengths differ from the recorded ones"));
    }
    let new_paras: Vec<Paragraph> = match op.get("content").and_then(Value::as_array) {
        Some(content) => content
            .iter()
            .map(|b| match serde_json::from_value::<Block>(b.clone()) {
                Ok(Block::Para(p)) => Ok(p),
                Ok(Block::Table(_)) => Err(fail("a table in a paragraph window")),
                Err(e) => Err(fail(&format!("content: {e}"))),
            })
            .collect::<Result<_, _>>()?,
        None => apply_text(&window_paras, offset, deleted, inserted).ok_or_else(|| fail("the recorded text change doesn't fit its blocks"))?,
    };
    // The window's text before, with the change made, must be the text the content has.
    let flow = |ps: &mut dyn Iterator<Item = &str>| ps.collect::<Vec<_>>().join(&PARA_MARK.to_string());
    let old_flow = flow(&mut window_paras.iter().map(|p| p.text.as_str()));
    let del_end = offset.checked_add(deleted.len()).ok_or_else(|| fail("bad offset"))?;
    if old_flow.get(offset..del_end) != Some(deleted) {
        return Err(fail("the deleted text differs from the recorded one"));
    }
    let want = format!("{}{inserted}{}", old_flow.get(..offset).unwrap_or_default(), old_flow.get(del_end..).unwrap_or_default());
    if flow(&mut new_paras.iter().map(|p| p.text.as_str())) != want
        || new_paras.len() != after.len()
        || new_paras.iter().zip(&after).any(|(p, (_, len))| p.text.len() != *len)
    {
        return Err(fail("the rebuilt blocks differ from the recorded ones"));
    }
    if let Some(story_content) = op.get("storyBlocks").and_then(Value::as_array) {
        // The window wasn't one run of neighbouring blocks (a table came or went): the op
        // carries the whole story.
        let bl: Blocks = story_content
            .iter()
            .map(|b| serde_json::from_value::<Block>(b.clone()).map(Arc::new).map_err(|e| fail(&format!("storyBlocks: {e}"))))
            .collect::<Result<_, _>>()?;
        *story_blocks_mut(&mut st.doc, story).ok_or_else(|| fail("no such story"))? = bl;
    } else {
        if !blocks::sibling_run(&window) {
            return Err(ReplayError::Unsupported { seq, what: "a change across a table's boundary without the story's blocks".into() });
        }
        let path = window.first().map(|(p, _)| p.clone()).unwrap_or_default();
        let at = path.last().copied().and_then(|i| usize::try_from(i).ok()).ok_or_else(|| fail("bad path"))?;
        let n = window.len();
        drop(window);
        let container = story_blocks_mut(&mut st.doc, story).and_then(|b| container_mut(b, &path)).ok_or_else(|| fail("can't reach its blocks"))?;
        if at + n > container.len() {
            return Err(fail("can't reach its blocks"));
        }
        container.splice(at..at + n, new_paras.into_iter().map(|p| Arc::new(Block::Para(p))));
    }
    let ids = st.ids.get_mut(&story).ok_or_else(|| fail("story went away"))?;
    ids.splice(first..first + before.len(), after.into_iter().map(|(id, _)| id));
    let count = story_blocks(&st.doc, story).map_or(0, |b| blocks::paragraphs(b).len());
    if st.ids.get(&story).map_or(0, Vec::len) != count {
        return Err(fail("the story's paragraphs and block ids no longer line up"));
    }
    Ok(())
}
