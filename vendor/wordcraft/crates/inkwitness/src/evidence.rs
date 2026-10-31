//! The evidence document (`schemas/evidence-v1.schema.json`, InkWitness repository) a writer
//! puts in its DOCX: who and what the attempt is, the recorded events and checkpoints, and the
//! export record.

use serde_json::{Value, json};
use wordcraft_doc::Document;

use crate::Recorder;

/// What the writer knows about the attempt beyond the recorder's [`Config`](crate::Config).
#[derive(Clone, Debug)]
pub struct Attempt {
    pub assignment_id: String,
    pub student_display_name: String,
    /// Whether the student typed the name in (G1 onboarding); `false` for a placeholder.
    pub identity_entered_by_student: bool,
    pub entered_at_local: String,
    pub policy_version: String,
    /// When the student acknowledged the policy (G1's first-export attestation).
    pub acknowledged_at_local: String,
    pub writer_version: String,
    pub started_at_local: String,
    /// Why this session started.
    pub reason: SessionReason,
    /// The sessions before this one, oldest first (empty for a first session). Each gap between
    /// two sessions is written as an unobserved span.
    pub earlier_sessions: Vec<EarlierSession>,
    /// Spans InkWitness did not see, beyond the gaps and the import this file works out itself
    /// (for example a change made outside the writer). Each is a schema `unobservedSpans` item.
    pub other_unobserved: Vec<Value>,
}

/// Why a writer session started (`sessionSpans[].reason`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionReason {
    New,
    Resume,
    Recovery,
    Import,
}

impl SessionReason {
    fn as_str(self) -> &'static str {
        match self {
            SessionReason::New => "new",
            SessionReason::Resume => "resume",
            SessionReason::Recovery => "recovery",
            SessionReason::Import => "import",
        }
    }
}

/// A session that ended before the current one, as the writer remembers it.
#[derive(Clone, Debug)]
pub struct EarlierSession {
    pub span_id: String,
    pub started_at_local: String,
    pub ended_at_local: String,
    pub started_elapsed_ms: u64,
    pub reason: SessionReason,
    /// The seq of the last event recorded in it (0 if none).
    pub last_seq: u64,
}

/// The evidence JSON for the recorder's session, `doc` being the document as exported.
pub fn document(r: &Recorder, doc: &Document, a: &Attempt, exported_at_local: &str) -> Value {
    let mut v = body(r, doc, a, exported_at_local);
    // The schema only has a `true` for this: a placeholder name leaves it out.
    if a.identity_entered_by_student
        && let Some(s) = v.get_mut("student").and_then(Value::as_object_mut)
    {
        s.insert("identityEnteredByStudent".into(), json!(true));
    }
    v
}

fn body(r: &Recorder, doc: &Document, a: &Attempt, exported_at_local: &str) -> Value {
    let config = r.config();
    let events = r.events();
    let section = config.section_id.clone().unwrap_or_else(|| "main".into());
    let (sessions, spans) = sessions_and_spans(a, &config, events.len());
    let version = if !a.earlier_sessions.is_empty() || !a.other_unobserved.is_empty() || a.reason != SessionReason::New {
        "1.2"
    } else if r.merges_typing() {
        "1.1"
    } else {
        "1.0"
    };
    let mut v = json!({
        "schema": "urn:workalone:evidence",
        "schemaVersion": version,
        "writerVersion": a.writer_version,
        "assignmentId": a.assignment_id,
        "attemptId": config.attempt_id,
        "student": {
            "displayName": a.student_display_name,
            "studentCode": null,
            "enteredAtLocal": a.entered_at_local,
            "identityAuthenticated": false
        },
        "policy": {
            "aiAssistance": "not-permitted",
            "clipboard": "allow-record",
            "policyVersion": a.policy_version,
            "acknowledgedAtLocal": a.acknowledged_at_local
        },
        "sections": [{"sectionId": section, "fields": [{"fieldId": config.field_id, "kind": "richtext"}]}],
        "sessionSpans": sessions,
        "events": events,
        "checkpoints": r.checkpoints(),
        "diagnostics": [],
        "export": {
            "finalSeq": events.len(),
            "exportedAtLocal": exported_at_local,
            "recordedStateDigest": r.state_digest(doc),
            "integrityType": "unsigned-local-chain",
            "selfHashCaveat": "Unsigned local evidence; not proof of authenticity"
        }
    });
    if version == "1.2"
        && let Some(o) = v.as_object_mut()
    {
        o.insert("unobservedSpans".into(), Value::Array(spans));
    }
    v
}

/// The sessions (`sessionSpans`) and the unobserved spans for the attempt. A gap between two
/// sessions is a span of unknown activity; text already there at an import is another. Nothing
/// is said about what happened in either.
fn sessions_and_spans(a: &Attempt, config: &crate::Config, event_count: usize) -> (Vec<Value>, Vec<Value>) {
    let mut sessions = Vec::new();
    let mut spans: Vec<Value> = Vec::new();
    // The session before, as (its number, id, end time, last seq).
    let mut prev: Option<(usize, &str, &str, u64)> = None;
    for (i, e) in a.earlier_sessions.iter().enumerate() {
        sessions.push(json!({
            "spanId": e.span_id, "startedAtLocal": e.started_at_local, "endedAtLocal": e.ended_at_local,
            "startedElapsedMs": e.started_elapsed_ms, "attemptId": config.attempt_id, "reason": e.reason.as_str(),
            "startedAtServer": null, "endedAtServer": null, "lastSeq": e.last_seq
        }));
        if let Some(p) = prev {
            spans.push(gap(spans.len(), p, (&e.span_id, &e.started_at_local, e.last_seq)));
        }
        prev = Some((i + 1, &e.span_id, &e.ended_at_local, e.last_seq));
    }
    let last = u64::try_from(event_count).unwrap_or(u64::MAX);
    sessions.push(json!({
        "spanId": config.session_span_id, "startedAtLocal": a.started_at_local, "startedElapsedMs": 0,
        "attemptId": config.attempt_id, "reason": a.reason.as_str(), "startedAtServer": null, "endedAtServer": null,
        "lastSeq": last
    }));
    if let Some(p) = prev {
        spans.push(gap(spans.len(), p, (&config.session_span_id, &a.started_at_local, last)));
    }
    if a.reason == SessionReason::Import {
        spans.push(json!({
            "spanId": format!("unobserved-import-{}", config.session_span_id), "kind": "preexisting-at-import",
            "label": "Not observed: the text already in the document when it was opened",
            "afterSeq": 0, "beforeSeq": (event_count > 0).then_some(1), "fromSessionSpanId": null,
            "toSessionSpanId": config.session_span_id, "startedAtLocal": null, "endedAtLocal": a.started_at_local,
            "startedAtServer": null, "endedAtServer": null, "fieldId": config.field_id,
            "startAnchor": null, "endAnchor": null
        }));
    }
    spans.extend(a.other_unobserved.iter().cloned());
    (sessions, spans)
}

/// The gap after `from` and before the session `to` (id, start time, last seq): it runs from the
/// last event of `from` to the first event of `to`, which is null if `to` recorded none.
fn gap(n: usize, from: (usize, &str, &str, u64), to: (&str, &str, u64)) -> Value {
    let (number, from_id, from_end, from_last) = from;
    json!({
        "spanId": format!("unobserved-gap-{}", n + 1), "kind": "gap-between-sessions",
        "label": format!("Not observed: between session {number} and session {}", number + 1),
        "afterSeq": from_last, "beforeSeq": (to.2 > from_last).then_some(from_last + 1),
        "fromSessionSpanId": from_id, "toSessionSpanId": to.0,
        "startedAtLocal": from_end, "endedAtLocal": to.1, "startedAtServer": null, "endedAtServer": null,
        "fieldId": null, "startAnchor": null, "endAnchor": null
    })
}
