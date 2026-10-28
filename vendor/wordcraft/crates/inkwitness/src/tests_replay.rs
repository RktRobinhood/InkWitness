//! Replay tests (backlog G0-6): rebuilding every recorded state, seeking to a paste, and the
//! checks that stop a replay instead of letting it guess.

use serde_json::{Value, json};
use wordcraft_doc::{Document, StoryRef};
use wordcraft_engine::Session;
use wordcraft_engine::observe::Gesture;

use super::replay::{Replay, ReplayError, State};
use super::*;

struct TestEnv {
    ms: u64,
    ids: u64,
}

impl Env for TestEnv {
    fn now_iso(&mut self) -> String {
        "2026-10-28T09:00:00Z".into()
    }
    fn monotonic_ms(&mut self) -> u64 {
        self.ms += 10;
        self.ms
    }
    fn uuid(&mut self) -> String {
        self.ids += 1;
        let mut b = [0u8; 16];
        b[8..].copy_from_slice(&self.ids.to_be_bytes());
        uuid_v4(b)
    }
}

const ATTEMPT: &str = "6f1c2a3b-4d5e-4f60-8a7b-9c0d1e2f3a4b";

fn config() -> Config {
    Config { attempt_id: ATTEMPT.into(), session_span_id: "span-1".into(), field_id: "essay".into(), section_id: None }
}

/// A recorded session that keeps every state it went through, to compare replays against.
struct Live {
    s: Session,
    r: Recorder,
    states: Vec<Document>,
}

impl Live {
    fn new(doc: Document) -> Live {
        let mut s = Session::new(doc);
        let r = Recorder::attach(&mut s, Box::new(TestEnv { ms: 0, ids: 0 }), config());
        let states = vec![s.doc.clone()];
        Live { s, r, states }
    }

    /// Run a command; one state per event it made.
    fn run(&mut self, id: &str, params: Value) {
        let n = self.r.events().len();
        self.s.run(id, &params).unwrap_or_else(|e| panic!("{id}: {e}"));
        let made = self.r.events().len() - n;
        assert!(made <= 1, "{id} made {made} events");
        if made == 1 {
            self.states.push(self.s.doc.clone());
        }
    }

    fn type_keys(&mut self, text: &str) {
        for c in text.chars() {
            self.run("text.insert", json!({"text": c.to_string()}));
        }
    }

    fn paste(&mut self, text: &str) {
        self.s.begin_gesture(Gesture::paste("edit.paste", Some(vec!["text/plain".into()])));
        self.run("edit.paste", json!({"text": text}));
        self.s.end_gesture();
    }

    fn evidence(&self) -> Value {
        json!({
            "events": self.r.events(),
            "checkpoints": self.r.checkpoints(),
            "export": {"recordedStateDigest": self.r.state_digest(&self.s.doc)},
        })
    }
}

/// Everything a replayed state must match: the stories and the recorded parts outside them.
fn assert_same(seq: usize, got: &Document, want: &Document) {
    assert_eq!(got.body, want.body, "body after event {seq}");
    assert_eq!(got.parts, want.parts, "parts after event {seq}");
    assert_eq!(got.last_section, want.last_section, "page setup after event {seq}");
    assert_eq!(got.styles, want.styles, "styles after event {seq}");
    assert_eq!(got.numbering, want.numbering, "numbering after event {seq}");
}

fn text_185() -> String {
    let s: String = "Example Student pasted this synthetic passage to test provenance. ".repeat(3).chars().take(185).collect();
    assert_eq!(s.len(), 185);
    s
}

/// The highlighted text when seeking to event `seq`: its after-anchor, resolved in the state
/// rebuilt up to it.
fn seek(rp: &Replay, seq: u64) -> (State, replay::Resolved) {
    let st = rp.state_at(seq).unwrap_or_else(|e| panic!("state_at({seq}): {e}"));
    let e = rp.event(seq).unwrap_or(&Value::Null);
    let window: Vec<String> = e
        .pointer("/ops/0/afterBlocks")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|b| b.get("blockId")?.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let anchor = e.get("afterAnchor").unwrap_or(&Value::Null);
    let res = st.resolve(anchor, Some(&window)).unwrap_or_else(|| panic!("event {seq}'s anchor doesn't resolve"));
    (st, res)
}

/// A session touching every kind of op: typing, a split, a 185-character paste, a deletion,
/// undo and redo, bold, a heading style, a font size, a margin change, a bulleted list, a
/// table, a merge and a cut.
fn busy_session() -> Live {
    let mut l = Live::new(Document::new());
    l.type_keys("Title");
    l.run("text.newParagraph", json!({}));
    l.type_keys("Intro: ");
    l.paste(&text_185());
    l.run("text.backspace", json!({}));
    l.run("edit.undo", json!({}));
    l.run("edit.redo", json!({}));
    l.run("edit.undo", json!({}));
    l.run("text.newParagraph", json!({}));
    l.type_keys("More text");
    l.run("select.all", json!({}));
    l.run("format.size", json!({"size": 14}));
    l.run("caret.docStart", json!({}));
    l.run("select.word", json!({}));
    l.run("format.bold", json!({}));
    l.run("para.style", json!({"style": "Heading 1"}));
    l.run("layout.margins", json!({"preset": "narrow"}));
    l.run("caret.docEnd", json!({}));
    l.run("text.newParagraph", json!({}));
    l.run("para.bullets", json!({}));
    l.type_keys("A point");
    l.run("insert.table", json!({"rows": 2, "cols": 2}));
    l.type_keys("cell");
    l.run("caret.docStart", json!({}));
    l.run("caret.end", json!({}));
    l.run("text.delete", json!({}));
    l.run("select.word", json!({}));
    l.run("edit.cut", json!({}));
    l
}

#[test]
fn replaying_every_event_reproduces_every_recorded_state() {
    let l = busy_session();
    let rp = Replay::from_evidence(&l.evidence()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(rp.events().len() + 1, l.states.len());
    // From the checkpoint to each state in turn, and every state on the way.
    for (k, want) in l.states.iter().enumerate() {
        let st = rp.state_at(k as u64).unwrap_or_else(|e| panic!("state_at({k}): {e}"));
        assert_same(k, &st.doc, want);
    }
    // One pass through all of them ends at the recorded final document.
    let end = rp.verify_all().unwrap_or_else(|e| panic!("{e}"));
    assert_same(l.states.len() - 1, &end.doc, &l.s.doc);
    assert_eq!(end.block_ids(), l.r.block_ids(), "the replayed block ids are the recorder's");
}

#[test]
fn replay_is_deterministic() {
    let a = busy_session();
    let b = busy_session();
    assert_eq!(a.r.events(), b.r.events(), "the same session records the same events");
    let ra = Replay::from_evidence(&a.evidence()).unwrap_or_else(|e| panic!("{e}"));
    let rb = Replay::from_evidence(&b.evidence()).unwrap_or_else(|e| panic!("{e}"));
    for k in 0..=ra.last_seq() {
        let (mut x, mut y) = (ra.state_at(k).unwrap_or_else(|e| panic!("{e}")), rb.state_at(k).unwrap_or_else(|e| panic!("{e}")));
        assert_eq!(x.doc, y.doc);
        assert_eq!(x.digest(), y.digest());
    }
}

#[test]
fn typing_carries_text_only_and_other_commands_their_content() {
    let l = busy_session();
    let ev = l.r.events();
    let typed: Vec<&Value> = ev.iter().filter(|e| e.pointer("/ops/0/command") == Some(&json!("text.insert"))).collect();
    assert!(typed.len() > 20);
    let plain = typed.iter().filter(|e| e.pointer("/ops/0/content").is_none()).count();
    // Typing into a table cell, a list item or after formatting may need its paragraph model:
    // whatever the rule can't rebuild carries content. Most keystrokes need none.
    assert!(plain * 10 >= typed.len() * 8, "{plain} of {} keystrokes carry text only", typed.len());
    let paste = ev.iter().find(|e| e["type"] == "paste").unwrap_or(&Value::Null);
    assert!(paste.pointer("/ops/0/content").is_some(), "a paste carries its paragraphs");
    let margins = ev.iter().find(|e| e.pointer("/ops/0/command") == Some(&json!("layout.margins"))).unwrap_or(&Value::Null);
    assert_eq!(margins.pointer("/ops/0/op"), Some(&json!("setPart")));
    assert_eq!(margins.pointer("/ops/0/part"), Some(&json!("lastSection")));
    let table = ev.iter().find(|e| e.pointer("/ops/0/command") == Some(&json!("insert.table"))).unwrap_or(&Value::Null);
    assert!(table.pointer("/ops/0/storyBlocks").is_some(), "a table coming carries its story");
}

#[test]
fn seeking_to_the_paste_highlights_exactly_the_pasted_text() {
    let l = busy_session();
    let rp = Replay::from_evidence(&l.evidence()).unwrap_or_else(|e| panic!("{e}"));
    let pastes = rp.pastes();
    assert_eq!(pastes.len(), 1);
    let seq = pastes[0];
    let e = rp.event(seq).unwrap_or(&Value::Null);
    assert!(e["gestureId"].is_string());
    let (st, res) = seek(&rp, seq);
    assert!(res.context_matches, "the text around the paste is the text the recorder hashed");
    assert_eq!(st.text(&res.start, &res.end), text_185());
    assert_eq!(res.start.story, StoryRef::Body);
    assert_eq!((res.start.off, res.end.off), (7, 192), "after `Intro: ` in the second paragraph");
}

#[test]
fn layout_changes_dont_move_the_flag() {
    // Margins, font size and paragraphs above all change before the paste is replayed to…
    let mut l = Live::new(Document::new());
    l.type_keys("First");
    l.run("layout.margins", json!({"preset": "wide"}));
    l.run("select.all", json!({}));
    l.run("format.size", json!({"size": 20}));
    l.run("caret.docEnd", json!({}));
    l.run("text.newParagraph", json!({}));
    l.paste(&text_185());
    l.run("caret.docStart", json!({}));
    l.run("text.newParagraph", json!({}));
    l.type_keys("Inserted above");
    let rp = Replay::from_evidence(&l.evidence()).unwrap_or_else(|e| panic!("{e}"));
    let seq = rp.pastes()[0];
    let (mut st, res) = seek(&rp, seq);
    assert_eq!(st.text(&res.start, &res.end), text_185());
    assert_eq!(st.doc.last_section.margin_left, l.s.doc.last_section.margin_left, "the replayed page has the recorded margins");
    // …and the reviewer's own layout changes after it: same model range, other geometry.
    let lay = |doc: &Document| wordcraft_layout::layout(doc, &mut wordcraft_layout::LayoutCache::new(), &Default::default());
    let before = lay(&st.doc).selection_rects(&st.doc, &res.start, &res.end, 0);
    st.doc.last_section.margin_left += 144.0;
    st.doc.last_section.margin_right += 144.0;
    let narrow = lay(&st.doc).selection_rects(&st.doc, &res.start, &res.end, 0);
    let again = st.resolve(&rp.event(seq).unwrap_or(&Value::Null)["afterAnchor"], None).unwrap_or_else(|| panic!("no anchor"));
    assert_eq!((&again.start, &again.end), (&res.start, &res.end), "the anchor names the same model range");
    assert_eq!(st.text(&again.start, &again.end), text_185());
    assert!(
        !before.is_empty() && narrow.len() > before.len(),
        "narrower text wraps the same range onto more lines ({} → {})",
        before.len(),
        narrow.len()
    );
    // The rest of the session still replays to the recorded end.
    let end = rp.verify_all().unwrap_or_else(|e| panic!("{e}"));
    assert_same(rp.events().len(), &end.doc, &l.s.doc);
}

#[test]
fn an_import_replays_from_its_own_checkpoint() {
    let mut l = Live::new(Document::new());
    l.type_keys("Old");
    l.run("file.new", json!({}));
    l.type_keys("New text");
    l.paste(&text_185());
    assert_eq!(l.r.checkpoints().len(), 2, "one on attach, one for the new document");
    let rp = Replay::from_evidence(&l.evidence()).unwrap_or_else(|e| panic!("{e}"));
    for (k, want) in l.states.iter().enumerate() {
        let st = rp.state_at(k as u64).unwrap_or_else(|e| panic!("state_at({k}): {e}"));
        assert_same(k, &st.doc, want);
    }
    let (st, res) = seek(&rp, rp.pastes()[0]);
    assert_eq!(st.text(&res.start, &res.end), text_185());
    // Without that checkpoint the import can't be crossed, and replay says so.
    let mut ev = l.evidence();
    if let Some(cps) = ev.get_mut("checkpoints").and_then(Value::as_array_mut) {
        cps.truncate(1);
    }
    let rp = Replay::from_evidence(&ev).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(rp.state_at(rp.last_seq()).err(), Some(ReplayError::NoCheckpoint(4)));
}

#[test]
fn altered_evidence_stops_the_replay() {
    let l = busy_session();
    let good = l.evidence();
    let seq = Replay::from_evidence(&good).unwrap_or_else(|e| panic!("{e}")).pastes()[0];
    let i = usize::try_from(seq - 1).unwrap_or(0);
    let try_with = |f: &dyn Fn(&mut Value)| {
        let mut ev = good.clone();
        if let Some(e) = ev.pointer_mut(&format!("/events/{i}")) {
            f(e);
        }
        Replay::from_evidence(&ev).and_then(|rp| rp.state_at(rp.last_seq())).err()
    };
    // Different pasted text: the content no longer fits the op.
    assert!(matches!(try_with(&|e| e["ops"][0]["inserted"] = json!("something else")), Some(ReplayError::Op { seq: s, .. }) if s == seq));
    // Different content with the same text length: the digest catches it.
    assert!(matches!(
        try_with(&|e| e["ops"][0]["content"][0]["text"] = json!(e["ops"][0]["content"][0]["text"].as_str().unwrap_or_default().replace('E', "F"))),
        Some(ReplayError::Op { .. } | ReplayError::Digest { .. })
    ));
    // A block that isn't there.
    assert!(matches!(try_with(&|e| e["ops"][0]["beforeBlocks"][0]["blockId"] = json!("b999")), Some(ReplayError::Op { .. })));
    // Its digest changed.
    assert_eq!(try_with(&|e| e["documentStateDigest"] = json!("0".repeat(64))), Some(ReplayError::Digest { seq }));
    // A gap in the numbering.
    assert!(matches!(try_with(&|e| e["seq"] = json!(999)), Some(ReplayError::Evidence(_))));
    // A corrupt checkpoint.
    let mut ev = good.clone();
    ev["checkpoints"][0]["compressedStateBase64"] = json!("AAAA");
    assert!(matches!(Replay::from_evidence(&ev).and_then(|rp| rp.state_at(1)).err(), Some(ReplayError::Checkpoint(_))));
}

#[test]
fn hostile_evidence_never_panics() {
    let l = busy_session();
    let good = l.evidence();
    let n = good["events"].as_array().map_or(0, Vec::len);
    let junk = [json!(null), json!(-1), json!("x"), json!([]), json!({}), json!(1e300), json!(u64::MAX)];
    for i in 0..n {
        for key in ["ops", "seq", "documentStateDigest", "afterAnchor"] {
            for j in &junk {
                let mut ev = good.clone();
                ev["events"][i][key] = j.clone();
                if let Ok(rp) = Replay::from_evidence(&ev) {
                    let _ = rp.state_at(rp.last_seq());
                }
            }
        }
        for field in ["offset", "deleted", "inserted", "beforeBlocks", "afterBlocks", "content", "storyBlocks", "story", "value"] {
            for j in &junk {
                let mut ev = good.clone();
                if let Some(op) = ev.pointer_mut(&format!("/events/{i}/ops/0")) {
                    op[field] = j.clone();
                }
                if let Ok(rp) = Replay::from_evidence(&ev) {
                    let _ = rp.state_at(rp.last_seq());
                }
            }
        }
    }
}

#[test]
fn the_docx_carries_a_replayable_history() {
    let l = busy_session();
    let payload = serde_json::to_vec(&json!({
        "attemptId": ATTEMPT,
        "events": l.r.events(),
        "checkpoints": l.r.checkpoints(),
        "export": {"recordedStateDigest": l.r.state_digest(&l.s.doc)},
    }))
    .unwrap_or_default();
    let mut doc = l.s.doc.clone();
    envelope::embed(&mut doc, ATTEMPT, &payload).unwrap_or_else(|e| panic!("{e}"));
    let bytes = wordcraft_docx::write(&doc).unwrap_or_else(|e| panic!("{e}"));
    let read = wordcraft_docx::read(&bytes).unwrap_or_else(|e| panic!("{e}"));
    let env = envelope::extract(&read).unwrap_or_else(|e| panic!("{e}")).unwrap_or_else(|| panic!("no envelope"));
    let evidence: Value = serde_json::from_slice(&env.payload).unwrap_or_default();
    let rp = Replay::from_evidence(&evidence).unwrap_or_else(|e| panic!("{e}"));
    let mut end = rp.verify_all().unwrap_or_else(|e| panic!("{e}"));
    // The DOCX's own text is the replayed final text (the writer's round trip is WordCraft's).
    assert_eq!(read.plain_text(StoryRef::Body), end.doc.plain_text(StoryRef::Body));
    assert_eq!(Some(end.digest().as_str()), rp.final_digest());
}
