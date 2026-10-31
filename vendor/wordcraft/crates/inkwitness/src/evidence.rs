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
    json!({
        "schema": "urn:workalone:evidence",
        "schemaVersion": if r.merges_typing() { "1.1" } else { "1.0" },
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
        "sessionSpans": [{
            "spanId": config.session_span_id,
            "startedAtLocal": a.started_at_local,
            "startedElapsedMs": 0,
            "attemptId": config.attempt_id,
            "reason": "new"
        }],
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
    })
}
