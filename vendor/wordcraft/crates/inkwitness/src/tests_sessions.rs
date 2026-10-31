//! Sessions and unobserved spans (schema 1.2, `docs/adr/0003-sessions-and-unobserved-spans.md`):
//! two sessions carry the gap between them, older files still open, and a fixture with an
//! unobserved span passes the digest chain and replay.

use serde_json::{Value, json};
use wordcraft_doc::Document;
use wordcraft_engine::Session;

use super::evidence::{self, Attempt, EarlierSession, SessionReason};
use super::replay::Replay;
use super::*;

const ATTEMPT: &str = "6f1c2a3b-4d5e-4f60-8a7b-9c0d1e2f3a4b";
const FIXTURE: &str = include_str!("../../../../../examples/evidence-v1.2-unobserved.json");

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

fn recorded() -> (Session, Recorder) {
    let mut s = Session::new(Document::new());
    let config = Config { attempt_id: ATTEMPT.into(), session_span_id: "session-2".into(), field_id: "essay".into(), section_id: Some("main".into()) };
    let r = Recorder::attach(&mut s, Box::new(TestEnv { ms: 0, ids: 0 }), config);
    for key in ["Hi", " there"] {
        let _ = s.run("text.insert", &json!({"text": key}));
    }
    (s, r)
}

fn attempt(earlier: Vec<EarlierSession>, reason: SessionReason) -> Attempt {
    Attempt {
        assignment_id: "essay".into(),
        student_display_name: "Example Student".into(),
        identity_entered_by_student: true,
        entered_at_local: "2026-10-28T09:00:00Z".into(),
        policy_version: "test".into(),
        acknowledged_at_local: "2026-10-28T09:00:00Z".into(),
        writer_version: "test".into(),
        started_at_local: "2026-10-29T09:00:00Z".into(),
        reason,
        earlier_sessions: earlier,
        other_unobserved: Vec::new(),
    }
}

fn session_one() -> EarlierSession {
    EarlierSession {
        span_id: "session-1".into(),
        started_at_local: "2026-10-28T09:00:00Z".into(),
        ended_at_local: "2026-10-28T09:40:00Z".into(),
        started_elapsed_ms: 0,
        reason: SessionReason::New,
        last_seq: 0,
    }
}

fn replays(evidence: &Value) -> Replay {
    let rp = Replay::from_evidence(evidence).unwrap_or_else(|e| panic!("{e}"));
    rp.verify_all().unwrap_or_else(|e| panic!("{e}"));
    rp
}

/// The digest chain of the events links from the attempt id, and each digest is of its event.
fn assert_chain(evidence: &Value) {
    let mut prior = sha256_hex(DOMAIN_GENESIS, ATTEMPT.as_bytes());
    for e in evidence["events"].as_array().cloned().unwrap_or_default() {
        assert_eq!(e["priorEventDigest"], prior.as_str());
        let mut bare = e.as_object().cloned().unwrap_or_default();
        bare.remove("eventDigest");
        prior = sha256_hex(DOMAIN_EVENT, canonical_json(&Value::Object(bare)).as_bytes());
        assert_eq!(e["eventDigest"], prior.as_str());
    }
}

#[test]
fn two_sessions_carry_two_sessions_and_the_gap_between_them() {
    let (s, r) = recorded();
    let v = evidence::document(&r, &s.doc, &attempt(vec![session_one()], SessionReason::Resume), "2026-10-29T10:00:00Z");
    assert_eq!(v["schemaVersion"], "1.2");
    let sessions = v["sessionSpans"].as_array().cloned().unwrap_or_default();
    assert_eq!(sessions.iter().map(|x| x["spanId"].clone()).collect::<Vec<_>>(), [json!("session-1"), json!("session-2")]);
    let spans = v["unobservedSpans"].as_array().cloned().unwrap_or_default();
    assert_eq!(spans.len(), 1);
    let gap = &spans[0];
    assert_eq!(gap["kind"], "gap-between-sessions");
    assert_eq!((gap["fromSessionSpanId"].as_str(), gap["toSessionSpanId"].as_str()), (Some("session-1"), Some("session-2")));
    // It starts where session 1 ended and ends where session 2 began, both on the client clock.
    assert_eq!((gap["startedAtLocal"].as_str(), gap["endedAtLocal"].as_str()), (Some("2026-10-28T09:40:00Z"), Some("2026-10-29T09:00:00Z")));
    assert_eq!((gap["afterSeq"].as_u64(), gap["beforeSeq"].as_u64()), (Some(0), Some(1)));
    // Server time is its own field and is empty in V1.
    for x in sessions.iter().chain(spans.iter()) {
        for k in ["startedAtServer", "endedAtServer"] {
            assert!(x.get(k).is_none_or(Value::is_null), "{k} is empty in V1");
        }
    }
    let label = gap["label"].as_str().unwrap_or_default();
    assert!(label.starts_with("Not observed"), "{label}");
    let rp = replays(&v);
    assert_eq!((rp.sessions().len(), rp.unobserved_spans().len()), (2, 1));
    assert_chain(&v);
}

#[test]
fn an_imported_start_names_the_text_already_there() {
    let (s, r) = recorded();
    let v = evidence::document(&r, &s.doc, &attempt(Vec::new(), SessionReason::Import), "2026-10-29T10:00:00Z");
    let spans = v["unobservedSpans"].as_array().cloned().unwrap_or_default();
    assert_eq!(spans.iter().map(|x| x["kind"].clone()).collect::<Vec<_>>(), [json!("preexisting-at-import")]);
    assert_eq!(spans[0]["beforeSeq"], 1);
}

#[test]
fn files_before_1_2_still_open_and_replay() {
    let (s, r) = recorded();
    let v = evidence::document(&r, &s.doc, &attempt(Vec::new(), SessionReason::New), "2026-10-29T10:00:00Z");
    assert_ne!(v["schemaVersion"], "1.2");
    assert!(v.get("unobservedSpans").is_none(), "an older file has no span list");
    let rp = replays(&v);
    assert_eq!(rp.sessions().len(), 1);
    assert!(rp.unobserved_spans().is_empty());
}

#[test]
fn the_fixture_with_an_unobserved_span_passes_the_digest_chain_and_replay() {
    let (s, r) = recorded();
    let mut a = attempt(vec![session_one()], SessionReason::Resume);
    a.other_unobserved.push(json!({
        "spanId": "unobserved-outside-1", "kind": "change-outside-writer",
        "label": "Not observed: a change made where InkWitness was not running",
        "afterSeq": 1, "beforeSeq": null, "fromSessionSpanId": null, "toSessionSpanId": null,
        "startedAtLocal": null, "endedAtLocal": null, "startedAtServer": null, "endedAtServer": null,
        "fieldId": "essay", "startAnchor": null, "endAnchor": null
    }));
    let v = evidence::document(&r, &s.doc, &a, "2026-10-29T10:00:00Z");
    // To rebuild the fixture: `INKWITNESS_DUMP_SESSIONS=<file> cargo test -p wordcraft-inkwitness sessions`.
    if let Ok(path) = std::env::var("INKWITNESS_DUMP_SESSIONS") {
        std::fs::write(path, serde_json::to_string_pretty(&v).unwrap_or_default() + "\n").unwrap_or_default();
    }
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap_or(Value::Null);
    assert_eq!(fixture, v, "the committed fixture is what the writer produces");
    let rp = replays(&fixture);
    let kinds: Vec<&str> = rp.unobserved_spans().iter().filter_map(|x| x["kind"].as_str()).collect();
    assert_eq!(kinds, ["gap-between-sessions", "change-outside-writer"]);
    assert_chain(&fixture);
}
