//! Opening a file in the writer (`docs/adr/0004-resuming-history.md`): read the history the
//! file carries and say whether the writer may continue it.
//!
//! A history is continued only when it is whole: the envelope reads, the events replay and
//! reach the recorded final state, and the digest chain links from the attempt id. Anything
//! else (no history, an unreadable one, another student's) starts a new attempt instead.

use std::collections::BTreeMap;

use serde_json::Value;
use wordcraft_doc::{Document, StoryRef};

use crate::evidence::{EarlierSession, SessionReason};
use crate::replay::Replay;
use crate::{DOMAIN_EVENT, DOMAIN_GENESIS, canonical_json, envelope, sha256_hex};

/// Who the writer is writing as: what a file's history has to match to be continued.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Who {
    pub student: String,
    pub assignment: String,
}

/// Why a file's history is not continued.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fresh {
    /// The file has no InkWitness history.
    NoHistory,
    /// There is a history, but it is invalid (the reason is in plain words).
    Corrupt(String),
    /// The history belongs to another student or assignment than the one writing.
    OtherStudent,
}

/// A history the writer continues: everything of it the recorder takes over.
#[derive(Debug)]
pub struct Continued {
    pub attempt_id: String,
    pub events: Vec<Value>,
    pub checkpoints: Vec<Value>,
    /// The digest of the last event (the genesis digest if there were none).
    pub prior: String,
    /// The paragraph ids at the recorded final state, and the highest `b<n>` ever used.
    pub ids: BTreeMap<StoryRef, Vec<String>>,
    pub used_ids: u64,
    /// `export.recordedStateDigest`, and the recorded final document's body text.
    pub recorded_digest: String,
    pub recorded_text: String,
    /// The sessions the file records, oldest first, and the spans to carry over (all but the
    /// gaps between sessions, which are worked out again).
    pub sessions: Vec<EarlierSession>,
    pub carried_spans: Vec<Value>,
    /// When the file's last session ended: its export time.
    pub last_ended_at: String,
    /// The identity the file records.
    pub student: String,
    pub assignment: String,
    pub entered_at_local: String,
    pub entered_by_student: bool,
}

pub enum Opening {
    Continue(Box<Continued>),
    Fresh(Fresh),
}

/// What opening `doc` should do, `who` being the identity the writer has (if it has one).
pub fn read(doc: &Document, who: Option<&Who>) -> Opening {
    let corrupt = |why: String| Opening::Fresh(Fresh::Corrupt(why));
    let env = match envelope::extract(doc) {
        Ok(Some(env)) => env,
        Ok(None) => return Opening::Fresh(Fresh::NoHistory),
        Err(e) => return corrupt(e.to_string()),
    };
    let evidence: Value = match serde_json::from_slice(&env.payload) {
        Ok(v) => v,
        Err(e) => return corrupt(e.to_string()),
    };
    match continued(&evidence, &env.attempt_id, who) {
        Ok(c) => Opening::Continue(Box::new(c)),
        Err(f) => Opening::Fresh(f),
    }
}

fn continued(evidence: &Value, envelope_attempt: &str, who: Option<&Who>) -> Result<Continued, Fresh> {
    let bad = |why: String| Fresh::Corrupt(why);
    let attempt_id = evidence.get("attemptId").and_then(Value::as_str).unwrap_or_default().to_string();
    if attempt_id != envelope_attempt || !envelope::is_uuid(&attempt_id) {
        return Err(bad("the attempt id is missing or differs from the envelope's".into()));
    }
    let replay = Replay::from_evidence(evidence).map_err(|e| bad(e.to_string()))?;
    let end = replay.verify_all().map_err(|e| bad(e.to_string()))?;
    if end.seq != replay.last_seq() {
        return Err(bad("the events don't replay to the end".into()));
    }
    let recorded_digest = replay.final_digest().ok_or_else(|| bad("no recorded final state".into()))?.to_string();
    let prior = verify_chain(replay.events(), &attempt_id).map_err(bad)?;
    let text = |p: &str| evidence.pointer(p).and_then(Value::as_str).map(str::trim).unwrap_or_default().to_string();
    let (student, assignment) = (text("/student/displayName"), text("/assignmentId"));
    if let Some(w) = who
        && (w.student.trim() != student || w.assignment.trim() != assignment)
    {
        return Err(Fresh::OtherStudent);
    }
    let exported = text("/export/exportedAtLocal");
    let sessions = sessions_of(evidence, replay.events(), &exported).map_err(bad)?;
    let carried_spans: Vec<Value> =
        replay.unobserved_spans().iter().filter(|s| s.get("kind").and_then(Value::as_str) != Some("gap-between-sessions")).cloned().collect();
    Ok(Continued {
        attempt_id,
        events: replay.events().to_vec(),
        checkpoints: evidence.get("checkpoints").and_then(Value::as_array).cloned().unwrap_or_default(),
        prior,
        ids: end.story_ids().clone(),
        used_ids: highest_id(replay.events(), &end),
        recorded_digest,
        recorded_text: end.doc.plain_text(StoryRef::Body),
        sessions,
        carried_spans,
        last_ended_at: exported,
        student,
        assignment,
        entered_at_local: text("/student/enteredAtLocal"),
        entered_by_student: evidence.pointer("/student/identityEnteredByStudent").and_then(Value::as_bool) == Some(true),
    })
}

/// Check that the events' digests chain from the attempt id's genesis and that each is the
/// digest of its own event; the last one's digest.
pub fn verify_chain(events: &[Value], attempt_id: &str) -> Result<String, String> {
    let mut prior = sha256_hex(DOMAIN_GENESIS, attempt_id.as_bytes());
    for e in events {
        let seq = e.get("seq").and_then(Value::as_u64).unwrap_or_default();
        if e.get("priorEventDigest").and_then(Value::as_str) != Some(prior.as_str()) {
            return Err(format!("event {seq} doesn't link to the one before it"));
        }
        if e.get("attemptId").and_then(Value::as_str) != Some(attempt_id) {
            return Err(format!("event {seq} belongs to another attempt"));
        }
        let mut bare = e.as_object().cloned().unwrap_or_default();
        let stored = bare.remove("eventDigest");
        prior = sha256_hex(DOMAIN_EVENT, canonical_json(&Value::Object(bare)).as_bytes());
        if stored.as_ref().and_then(Value::as_str) != Some(prior.as_str()) {
            return Err(format!("event {seq}'s digest doesn't match its content"));
        }
    }
    Ok(prior)
}

fn sessions_of(evidence: &Value, events: &[Value], exported: &str) -> Result<Vec<EarlierSession>, String> {
    let list = evidence.get("sessionSpans").and_then(Value::as_array).ok_or("no sessions")?;
    let mut out: Vec<EarlierSession> = Vec::new();
    for (i, s) in list.iter().enumerate() {
        let field = |k: &str| s.get(k).and_then(Value::as_str).map(str::to_string);
        let span_id = field("spanId").ok_or("a session has no id")?;
        // Older files give `lastSeq` only by their events: the last one of this session, or where
        // the session before ended when it recorded none.
        let by_events = events
            .iter()
            .filter(|e| e.get("sessionSpanId").and_then(Value::as_str) == Some(span_id.as_str()))
            .filter_map(|e| e.get("seq").and_then(Value::as_u64))
            .max();
        let last_seq = s.get("lastSeq").and_then(Value::as_u64).or(by_events).unwrap_or_else(|| out.last().map_or(0, |p| p.last_seq));
        let last = i + 1 == list.len();
        let ended = field("endedAtLocal").or_else(|| last.then(|| exported.to_string())).or_else(|| field("startedAtLocal")).unwrap_or_default();
        let reason = match s.get("reason").and_then(Value::as_str) {
            Some("resume") => SessionReason::Resume,
            Some("recovery") => SessionReason::Recovery,
            Some("import") => SessionReason::Import,
            _ => SessionReason::New,
        };
        out.push(EarlierSession {
            span_id,
            started_at_local: field("startedAtLocal").ok_or("a session has no start time")?,
            ended_at_local: ended,
            started_elapsed_ms: s.get("startedElapsedMs").and_then(Value::as_u64).unwrap_or(0),
            reason,
            last_seq,
        });
    }
    Ok(out)
}

/// The highest `b<n>` block id the history ever names: the paragraphs now, and every block of
/// every op (those that have gone too), so a new paragraph never takes a used id.
fn highest_id(events: &[Value], end: &crate::replay::State) -> u64 {
    fn number(id: &str) -> u64 {
        id.strip_prefix('b').and_then(|n| n.parse().ok()).unwrap_or(0)
    }
    let mut top = end.story_ids().values().flatten().map(|id| number(id)).max().unwrap_or(0);
    for op in events.iter().filter_map(|e| e.get("ops").and_then(Value::as_array)).flatten() {
        for key in ["beforeBlocks", "afterBlocks"] {
            for b in op.get(key).and_then(Value::as_array).into_iter().flatten() {
                top = top.max(b.get("blockId").and_then(Value::as_str).map_or(0, number));
            }
        }
        if let Some(stories) = op.get("stories").and_then(Value::as_object) {
            top = top.max(stories.values().filter_map(Value::as_array).flatten().filter_map(Value::as_str).map(number).max().unwrap_or(0));
        }
    }
    top
}

/// Plain words for a [`Fresh`], shown to the student.
pub fn notice(f: &Fresh) -> String {
    match f {
        Fresh::NoHistory => "This file has no InkWitness history, so it opens as a new attempt. Its text is marked as already there and not observed.".into(),
        Fresh::Corrupt(why) => format!("The history in this file can't be used ({why}), so it was not continued. It opens as a new attempt, and its text is marked as already there and not observed."),
        Fresh::OtherStudent => "This file's history belongs to a different student or assignment, so it was not continued. It opens as a new attempt, and its text is marked as already there and not observed.".into(),
    }
}

/// The `unobservedSpans` item for a change made where InkWitness was not running.
pub fn outside_change(n: usize, from: &Continued, to_span: &str, now: &str, field_id: &str) -> Value {
    let last = u64::try_from(from.events.len()).unwrap_or(u64::MAX);
    let from_id = from.sessions.last().map(|s| s.span_id.clone());
    serde_json::json!({
        "spanId": format!("unobserved-outside-{n}"), "kind": "change-outside-writer",
        "label": format!("Not observed: the text changed outside the writer between session {} and session {}", from.sessions.len(), from.sessions.len() + 1),
        "afterSeq": last, "beforeSeq": last + 1, "fromSessionSpanId": from_id, "toSessionSpanId": to_span,
        "startedAtLocal": from.last_ended_at, "endedAtLocal": now, "startedAtServer": null, "endedAtServer": null,
        "fieldId": field_id, "startAnchor": null, "endAnchor": null
    })
}
