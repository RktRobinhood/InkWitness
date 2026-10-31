//! Keeping history across sessions (`docs/adr/0004-resuming-history.md`, GH #9): a file opened in
//! the writer continues the history it carries, or starts a new attempt, and never mixes two
//! students' attempts.

use base64::Engine as _;
use serde_json::{Value, json};
use wordcraft_doc::{Document, StoryRef};
use wordcraft_engine::Session;

use super::evidence::{self, Attempt, SessionReason};
use super::replay::Replay;
use super::resume::verify_chain;
use super::*;

/// A clock that moves on, so each session is on its own day.
struct DayEnv {
    day: u32,
    ms: u64,
    ids: u64,
}

impl Env for DayEnv {
    fn now_iso(&mut self) -> String {
        format!("2026-10-{:02}T09:00:00Z", self.day)
    }
    fn monotonic_ms(&mut self) -> u64 {
        self.ms += 10;
        self.ms
    }
    fn uuid(&mut self) -> String {
        self.ids += 1;
        let mut b = [0u8; 16];
        b[7] = u8::try_from(self.day).unwrap_or(0);
        b[8..].copy_from_slice(&self.ids.to_be_bytes());
        uuid_v4(b)
    }
}

/// A writer page: a session with a recorder on `day`, writing as `who` if given. Saving is what
/// `writer::attach` does, here by hand (`save`).
struct Page {
    s: Session,
    r: Recorder,
}

fn page(day: u32, who: Option<(&str, &str)>) -> Page {
    let mut s = Session::new(Document::new());
    let mut env = DayEnv { day, ms: 0, ids: 0 };
    let config = Config { attempt_id: env.uuid(), session_span_id: env.uuid(), field_id: "essay".into(), section_id: Some("main".into()) };
    let r = Recorder::attach(&mut s, Box::new(env), config);
    if let Some((student, assignment)) = who {
        r.set_identity(student, assignment);
    }
    Page { s, r }
}

fn type_text(p: &mut Page, text: &str) {
    for c in text.chars() {
        let _ = p.s.run("text.insert", &json!({"text": c.to_string()}));
    }
}

fn base_attempt(p: &Page, student: &str, assignment: &str) -> Attempt {
    let mut a = Attempt {
        assignment_id: assignment.into(),
        student_display_name: student.into(),
        identity_entered_by_student: true,
        entered_at_local: "2026-10-01T09:00:00Z".into(),
        policy_version: "test".into(),
        acknowledged_at_local: "2026-10-01T09:00:00Z".into(),
        writer_version: "test".into(),
        started_at_local: "2026-10-01T09:00:00Z".into(),
        reason: SessionReason::New,
        earlier_sessions: Vec::new(),
        other_unobserved: Vec::new(),
    };
    if let Some(o) = p.r.opened() {
        a.reason = o.reason;
        a.started_at_local = o.started_at_local;
        a.earlier_sessions = o.earlier_sessions;
        a.other_unobserved = o.other_unobserved;
        if let Some((student, assignment, entered_at, by)) = o.identity {
            a.student_display_name = student;
            a.assignment_id = assignment;
            a.entered_at_local = entered_at;
            a.identity_entered_by_student = by;
        }
    }
    a
}

/// Save the page as the writer does: the evidence embedded, the DOCX bytes.
fn save(p: &Page, student: &str, assignment: &str) -> Vec<u8> {
    let payload = evidence::document(&p.r, &p.s.doc, &base_attempt(p, student, assignment), "2026-10-30T12:00:00Z");
    let mut doc = p.s.doc.clone();
    envelope::embed(&mut doc, &p.r.config().attempt_id, &serde_json::to_vec(&payload).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    wordcraft_docx::write(&doc).unwrap_or_else(|e| panic!("{e}"))
}

fn open(p: &mut Page, bytes: &[u8]) {
    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
    p.s.run("file.open", &json!({"path": "essay.docx", "data": data})).unwrap_or_else(|e| panic!("{e}"));
}

fn evidence_of(bytes: &[u8]) -> Value {
    let doc = wordcraft_docx::read(bytes).unwrap_or_else(|e| panic!("{e}"));
    let env = envelope::extract(&doc).unwrap_or_else(|e| panic!("{e}")).unwrap_or_else(|| panic!("no history"));
    serde_json::from_slice(&env.payload).unwrap_or_default()
}

fn list(v: &Value, key: &str) -> Vec<Value> {
    v.get(key).and_then(Value::as_array).cloned().unwrap_or_default()
}

/// The history replays to its recorded end, and its digests chain from the attempt id.
fn assert_whole(v: &Value) {
    let rp = Replay::from_evidence(v).unwrap_or_else(|e| panic!("{e}"));
    rp.verify_all().unwrap_or_else(|e| panic!("{e}"));
    verify_chain(&list(v, "events"), v["attemptId"].as_str().unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
}

fn kinds(v: &Value) -> Vec<String> {
    list(v, "unobservedSpans").iter().filter_map(|s| s["kind"].as_str().map(str::to_string)).collect()
}

const A: (&str, &str) = ("Example Student", "essay");

#[test]
fn write_save_reopen_write_save_keeps_both_sessions_in_one_chain() {
    let mut one = page(28, Some(A));
    type_text(&mut one, "First day. ");
    let day_one = save(&one, A.0, A.1);
    let first = evidence_of(&day_one);

    // A later visit: a new page (its own attempt id), the file opened, more written.
    let mut two = page(30, Some(A));
    open(&mut two, &day_one);
    assert_eq!(two.s.doc.plain_text(StoryRef::Body), "First day. ");
    let _ = two.s.run("caret.docEnd", &json!({}));
    type_text(&mut two, "Second day.");
    let day_two = save(&two, A.0, A.1);
    let v = evidence_of(&day_two);

    assert_eq!(v["attemptId"], first["attemptId"], "the attempt carries on");
    assert_eq!(v["schemaVersion"], "1.2");
    assert_whole(&v);
    // The earlier events and their digests are as they were, in order.
    let (old, now) = (list(&first, "events"), list(&v, "events"));
    assert!(now.len() > old.len());
    assert_eq!(&now[..old.len()], &old[..], "the earlier events are untouched");
    // The new ones are the second session's and continue the numbering and the chain.
    let sessions = list(&v, "sessionSpans");
    assert_eq!(sessions.len(), 2);
    for (i, e) in now.iter().enumerate() {
        assert_eq!(e["seq"].as_u64(), u64::try_from(i + 1).ok());
    }
    let new_session = sessions[1]["spanId"].clone();
    assert!(now[old.len()..].iter().all(|e| e["sessionSpanId"] == new_session));
    assert!(now[..old.len()].iter().all(|e| e["sessionSpanId"] == sessions[0]["spanId"]));
    assert_eq!(now[old.len()]["priorEventDigest"], old[old.len() - 1]["eventDigest"]);
    // Only the gap between the sessions is unobserved: the file was where the history left it.
    assert_eq!(kinds(&v), ["gap-between-sessions"]);
    assert!(!now[old.len()..].iter().any(|e| e["type"] == "import_document"), "an exact continuation needs no boundary event");
    assert_eq!(v["export"]["finalSeq"].as_u64(), u64::try_from(now.len()).ok());
    // The final text is both days'.
    let end = Replay::from_evidence(&v).and_then(|r| r.verify_all()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(end.doc.plain_text(StoryRef::Body), "First day. Second day.");
}

#[test]
fn three_sessions_keep_two_gaps_and_ids_never_repeat() {
    let mut bytes = {
        let mut p = page(20, Some(A));
        type_text(&mut p, "One");
        let _ = p.s.run("text.newParagraph", &json!({}));
        save(&p, A.0, A.1)
    };
    for day in [21, 22] {
        let mut p = page(day, Some(A));
        open(&mut p, &bytes);
        let _ = p.s.run("text.newParagraph", &json!({}));
        type_text(&mut p, "more");
        bytes = save(&p, A.0, A.1);
    }
    let v = evidence_of(&bytes);
    assert_whole(&v);
    assert_eq!(list(&v, "sessionSpans").len(), 3);
    assert_eq!(kinds(&v), ["gap-between-sessions", "gap-between-sessions"]);
    // Every event of a paragraph that was made after the first session has its own block id.
    let ids: Vec<String> = list(&v, "events")
        .iter()
        .flat_map(|e| list(e, "ops"))
        .flat_map(|o| list(&o, "afterBlocks"))
        .filter_map(|b| b["blockId"].as_str().map(str::to_string))
        .collect();
    assert!(ids.iter().any(|i| i == "b3"), "{ids:?}");
}

#[test]
fn a_change_made_in_word_is_one_unobserved_span_and_no_typing() {
    let mut one = page(28, Some(A));
    type_text(&mut one, "Typed in the writer.");
    let saved = save(&one, A.0, A.1);
    let first = evidence_of(&saved);

    // Word: the body changes, the evidence part stays (a re-save that keeps it).
    let mut doc = wordcraft_docx::read(&saved).unwrap_or_else(|e| panic!("{e}"));
    let mut word = Session::new(doc.clone());
    let _ = word.run("caret.docEnd", &json!({}));
    let _ = word.run("text.insert", &json!({"text": " Added in Word, a lot of it."}));
    doc.body = word.doc.body.clone();
    let edited = wordcraft_docx::write(&doc).unwrap_or_else(|e| panic!("{e}"));

    let mut two = page(30, Some(A));
    open(&mut two, &edited);
    let _ = two.s.run("caret.docEnd", &json!({}));
    type_text(&mut two, " More.");
    let v = evidence_of(&save(&two, A.0, A.1));
    assert_whole(&v);
    assert_eq!(v["attemptId"], first["attemptId"]);
    assert_eq!(kinds(&v).iter().filter(|k| *k == "change-outside-writer").count(), 1);
    let span = list(&v, "unobservedSpans").into_iter().find(|s| s["kind"] == "change-outside-writer").unwrap_or(Value::Null);
    let old = list(&first, "events").len();
    assert_eq!(span["afterSeq"].as_u64(), u64::try_from(old).ok());
    assert!(span["label"].as_str().unwrap_or_default().starts_with("Not observed"));
    // Between the old events and the new ones there is the one opening marker, and no event
    // carries any of the Word text as typing.
    let events = list(&v, "events");
    assert_eq!(events[old]["type"], "import_document");
    assert!(events.iter().all(|e| !e["insertedText"].as_str().unwrap_or_default().contains("Word")), "Word's text is not an event");
    assert!(events[old + 1..].iter().all(|e| e["type"] != "import_document"));
    let end = Replay::from_evidence(&v).and_then(|r| r.verify_all()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(end.doc.plain_text(StoryRef::Body), "Typed in the writer. Added in Word, a lot of it. More.");
}

/// Evidence with one event's text changed, so its digest no longer matches.
fn corrupted(bytes: &[u8]) -> Vec<u8> {
    let mut v = evidence_of(bytes);
    v["events"][0]["origin"] = json!("clipboard");
    let mut doc = wordcraft_docx::read(bytes).unwrap_or_else(|e| panic!("{e}"));
    let attempt = v["attemptId"].as_str().unwrap_or_default().to_string();
    envelope::embed(&mut doc, &attempt, &serde_json::to_vec(&v).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    wordcraft_docx::write(&doc).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn a_corrupt_history_is_not_continued_and_the_writer_says_so() {
    let mut one = page(28, Some(A));
    type_text(&mut one, "Some words.");
    let bad = corrupted(&save(&one, A.0, A.1));
    let old_attempt = evidence_of(&bad)["attemptId"].clone();

    let mut two = page(30, Some(A));
    open(&mut two, &bad);
    let notes = two.r.take_notices();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("can't be used") && notes[0].contains("new attempt"), "{}", notes[0]);
    type_text(&mut two, " Then more.");
    let v = evidence_of(&save(&two, A.0, A.1));
    assert_whole(&v);
    assert_ne!(v["attemptId"], old_attempt, "a new attempt");
    assert_eq!(list(&v, "sessionSpans").len(), 1);
    assert_eq!(list(&v, "events")[0]["type"], "import_document");
    assert_eq!(kinds(&v), ["preexisting-at-import"]);
    // Nothing of the unreadable history came along.
    assert!(!list(&v, "events").iter().any(|e| e["origin"] == "clipboard"));
}

#[test]
fn a_file_without_history_opens_as_a_new_attempt_with_its_text_unobserved() {
    let mut doc = Document::new();
    let mut plain = Session::new(doc.clone());
    let _ = plain.run("text.insert", &json!({"text": "From elsewhere."}));
    doc.body = plain.doc.body.clone();
    let bytes = wordcraft_docx::write(&doc).unwrap_or_else(|e| panic!("{e}"));

    let mut p = page(30, Some(A));
    type_text(&mut p, "Typed before opening.");
    let before = p.r.config().attempt_id;
    open(&mut p, &bytes);
    assert_ne!(p.r.config().attempt_id, before);
    assert!(p.r.take_notices()[0].contains("no InkWitness history"));
    let v = evidence_of(&save(&p, A.0, A.1));
    assert_whole(&v);
    assert_eq!(kinds(&v), ["preexisting-at-import"]);
    // The page's earlier typing is not in this attempt, and this file's text is an import.
    let events = list(&v, "events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["type"], "import_document");
    let end = Replay::from_evidence(&v).and_then(|r| r.verify_all()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(end.doc.plain_text(StoryRef::Body), "From elsewhere.");
}

/// The same-browser history leak of an earlier build (a previous student's events arriving in the
/// next student's file) must not come back (AGENTS.md: identity isolation).
#[test]
fn another_students_file_does_not_join_an_attempt_in_the_same_browser() {
    let mut a = page(28, Some(A));
    type_text(&mut a, "Student A's essay.");
    let file_a = save(&a, A.0, A.1);
    let evidence_a = evidence_of(&file_a);

    // Student B's page: B has written something, then opens A's file.
    let b = ("Other Student", "essay");
    let mut p = page(30, Some(b));
    type_text(&mut p, "B's own words.");
    open(&mut p, &file_a);
    let notes = p.r.take_notices();
    assert!(notes[0].contains("different student"), "{notes:?}");
    type_text(&mut p, " B goes on.");
    let v = evidence_of(&save(&p, b.0, b.1));
    assert_whole(&v);
    assert_eq!(v["student"]["displayName"], "Other Student");
    assert_ne!(v["attemptId"], evidence_a["attemptId"], "A's attempt isn't joined");
    let a_ids: Vec<Value> = list(&evidence_a, "events").iter().map(|e| e["eventId"].clone()).collect();
    assert!(list(&v, "events").iter().all(|e| !a_ids.contains(&e["eventId"])), "none of A's events are in B's file");
    let text = serde_json::to_string(&v).unwrap_or_default();
    assert!(!text.contains("Student A's"), "A's text didn't travel into B's evidence");
    assert!(!text.contains("B's own words"), "nor did the page's earlier attempt");

    // The same student's file is continued even from a page that wrote something first.
    let mut again = page(31, Some(A));
    open(&mut again, &file_a);
    assert_eq!(again.r.config().attempt_id, evidence_a["attemptId"].as_str().unwrap_or_default());
}

/// Growth over sessions: each one adds its own events and a few hundred bytes of session and gap
/// records. Recorded here, and bounded by the envelope's limits.
#[test]
fn history_stays_inside_the_size_limits_over_many_sessions() {
    let mut bytes = Vec::new();
    let mut sizes = Vec::new();
    for day in 1..=12u32 {
        let mut p = page(day, Some(A));
        if !bytes.is_empty() {
            open(&mut p, &bytes);
        }
        // About a page of typing a session.
        type_text(&mut p, &"Words typed on one more day of the essay. ".repeat(12));
        bytes = save(&p, A.0, A.1);
        let doc = wordcraft_docx::read(&bytes).unwrap_or_else(|e| panic!("{e}"));
        let part = wordcraft_docx::find_custom_xml_part(&doc, envelope::NAMESPACE).unwrap_or(None).map_or(0, |x| x.bytes.len());
        sizes.push((bytes.len(), part));
    }
    eprintln!("session: (docx bytes, evidence part bytes)\n{sizes:?}");
    let v = evidence_of(&bytes);
    assert_whole(&v);
    assert_eq!(list(&v, "sessionSpans").len(), 12);
    let (docx, part) = sizes[11];
    assert!(part < envelope::MAX_ENVELOPE_BYTES / 8, "{part}");
    assert!(docx < 8 << 20, "{docx}");
    // Growth is about linear: the last session adds no more than twice what the second did.
    let step = |i: usize| sizes[i].1 - sizes[i - 1].1;
    assert!(step(11) < 2 * step(1) + 4096, "{} vs {}", step(11), step(1));
}
