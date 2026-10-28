//! The InkWitness G0 event tests (backlog G0-3 and G0-4; TEST_MATRIX W01, W02, W04, W05).

use serde_json::{Value, json};
use wordcraft_doc::{Document, StoryRef};
use wordcraft_engine::Session;
use wordcraft_engine::observe::Gesture;

use super::*;

/// A fixed clock and counting ids, so events are deterministic.
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

fn fresh() -> (Session, Recorder) {
    let mut s = Session::new(Document::new());
    let r = Recorder::attach(&mut s, Box::new(TestEnv { ms: 0, ids: 0 }), config());
    (s, r)
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.run(id, &params).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn type_keys(s: &mut Session, text: &str) {
    for c in text.chars() {
        run(s, "text.insert", json!({"text": c.to_string()}));
    }
}

fn body(s: &Session) -> String {
    s.doc.plain_text(StoryRef::Body)
}

fn ids(r: &Recorder) -> Vec<String> {
    r.block_ids().get("body").cloned().unwrap_or_default()
}

fn last(r: &Recorder) -> Value {
    r.events().last().cloned().unwrap_or(Value::Null)
}

/// 185 characters of synthetic text, no line breaks.
fn text_185() -> String {
    let s: String = "Example Student pasted this synthetic passage to test provenance. ".repeat(3).chars().take(185).collect();
    assert_eq!(s.chars().count(), 185);
    assert_eq!(s.len(), 185, "ASCII: bytes and characters agree");
    s
}

#[test]
fn typing_a_paragraph_gives_one_insert_per_keystroke() {
    let (mut s, r) = fresh();
    type_keys(&mut s, "Knowledge matters");
    let ev = r.events();
    assert_eq!(ev.len(), 17);
    for (i, (e, c)) in ev.iter().zip("Knowledge matters".chars()).enumerate() {
        assert_eq!(e["seq"], i + 1);
        assert_eq!(e["type"], "text_insert");
        assert_eq!(e["origin"], "keyboard");
        assert_eq!(e["insertedText"], c.to_string());
        assert_eq!(e["deletedText"], "");
        assert_eq!(e["gestureId"], Value::Null);
        assert_eq!(e["afterAnchor"]["blockId"], "b1");
        assert_eq!(e["afterAnchor"]["startOffset"], i);
        assert_eq!(e["afterAnchor"]["endOffset"], i + 1);
        assert_eq!(e["beforeAnchor"]["startOffset"], i);
        assert_eq!(e["beforeAnchor"]["endOffset"], i);
        assert_eq!(e["ops"].as_array().map(Vec::len), Some(1));
        assert!(e["ops"][0].get("content").is_none(), "typing carries its text only");
    }
    assert_eq!(body(&s), "Knowledge matters");
}

#[test]
fn paste_of_185_characters_links_its_gesture_to_exactly_its_operations() {
    let (mut s, r) = fresh();
    type_keys(&mut s, "Intro ");
    let text = text_185();
    // The front end raises the gesture at the paste, then runs the paste command.
    let gid = s.begin_gesture(Gesture::paste("edit.paste", Some(vec!["text/plain".into()])));
    run(&mut s, "edit.paste", json!({"text": text}));
    s.end_gesture();
    let gid = gid.unwrap_or_default();
    assert!(!gid.is_empty(), "the recorder minted the gesture id");
    assert_eq!(body(&s), format!("Intro {text}"));

    let ev = r.events();
    assert_eq!(ev.len(), 7, "six keystrokes and exactly one paste event");
    let pastes: Vec<&Value> = ev.iter().filter(|e| e["type"] == "paste").collect();
    assert_eq!(pastes.len(), 1);
    let e = pastes[0];
    assert_eq!(e["gestureId"], gid.as_str());
    assert_eq!(e["pasteGestureId"], gid.as_str());
    assert_eq!(e["origin"], "clipboard");
    assert_eq!(e["clipboardTypes"], json!(["text/plain"]));
    assert_eq!(e["sourceStatus"], "unknown");
    assert_eq!(e["sourceHint"], Value::Null);
    assert_eq!(e["insertedText"], text.as_str());
    assert_eq!(e["deletedText"], "");
    // The operations cover exactly those 185 characters, at the right place.
    let ops = e["ops"].as_array().cloned().unwrap_or_default();
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0]["inserted"], text.as_str());
    assert_eq!(ops[0]["deleted"], "");
    assert_eq!(ops[0]["offset"], 6);
    assert_eq!(ops[0]["command"], "edit.paste");
    assert_eq!(
        e["beforeAnchor"],
        json!({"blockId": "b1", "startOffset": 6, "endOffset": 6, "contextHash": e["beforeAnchor"]["contextHash"], "state": "before"})
    );
    assert_eq!(e["afterAnchor"]["blockId"], "b1");
    assert_eq!(e["afterAnchor"]["startOffset"], 6);
    assert_eq!(e["afterAnchor"]["endOffset"], 191);
    // A paste carries the pasted paragraph's model, for an exact replay.
    assert!(ops[0]["content"].is_array());
    // No other event carries the gesture.
    assert_eq!(ev.iter().filter(|e| e["gestureId"] == gid.as_str()).count(), 1);
}

#[test]
fn a_gesture_never_reaches_macros_repeat_or_another_command() {
    let (mut s, r) = fresh();
    run(&mut s, "tools.recordMacro", json!({"name": "m"}));
    let gid = s.begin_gesture(Gesture::paste("edit.paste", None));
    run(&mut s, "edit.paste", json!({"text": "abc"}));
    s.end_gesture();
    run(&mut s, "tools.recordMacro", json!({}));
    // The command's params are the paste's own: nothing about the gesture.
    let steps = s.macros.get("m").cloned().unwrap_or_default();
    assert_eq!(steps, vec![("edit.paste".to_string(), json!({"text": "abc"}))]);
    assert_eq!(s.last_command, Some(("edit.paste".to_string(), json!({"text": "abc"}))));
    assert_eq!(last(&r)["gestureId"], json!(gid));
    assert!(last(&r).get("clipboardTypes").is_none(), "unknown clipboard types are left out, not guessed");

    // Repeat pastes again, without a gesture: an event of the Repeat command.
    run(&mut s, "edit.repeat", json!({}));
    assert_eq!(body(&s), "abcabc");
    assert_eq!(last(&r)["ops"][0]["command"], "edit.repeat");
    assert_eq!(last(&r)["gestureId"], Value::Null);
    assert!(last(&r).get("pasteGestureId").is_none());

    // A gesture raised for a paste isn't taken by a different command, and is gone after.
    s.begin_gesture(Gesture::paste("edit.paste", None));
    run(&mut s, "text.insert", json!({"text": "x"}));
    s.end_gesture();
    run(&mut s, "edit.paste", json!({"text": "y"}));
    let ev = r.events();
    let n = ev.len();
    assert_eq!(ev[n - 2]["type"], "text_insert");
    assert_eq!(ev[n - 2]["gestureId"], Value::Null);
    assert_eq!(ev[n - 1]["type"], "paste");
    assert_eq!(ev[n - 1]["gestureId"], Value::Null);
}

#[test]
fn delete_undo_and_redo() {
    let (mut s, r) = fresh();
    run(&mut s, "text.insert", json!({"text": "one two three"}));
    run(&mut s, "select.text", json!({"text": "two "}));
    run(&mut s, "text.delete", json!({}));
    assert_eq!(body(&s), "one three");
    let e = last(&r);
    assert_eq!(e["type"], "text_delete");
    assert_eq!(e["deletedText"], "two ");
    assert_eq!(e["beforeAnchor"]["blockId"], "b1");
    assert_eq!(e["beforeAnchor"]["startOffset"], 4);
    assert_eq!(e["beforeAnchor"]["endOffset"], 8);
    assert_eq!(e["afterAnchor"]["startOffset"], 4);
    assert_eq!(e["afterAnchor"]["endOffset"], 4);

    run(&mut s, "edit.undo", json!({}));
    assert_eq!(body(&s), "one two three");
    let e = last(&r);
    assert_eq!(e["type"], "undo");
    assert_eq!(e["origin"], "undo");
    assert_eq!(e["insertedText"], "two ");
    assert_eq!(e["afterAnchor"]["blockId"], "b1");
    assert_eq!(e["afterAnchor"]["startOffset"], 4);
    assert_eq!(e["afterAnchor"]["endOffset"], 8);

    run(&mut s, "edit.redo", json!({}));
    assert_eq!(body(&s), "one three");
    let e = last(&r);
    assert_eq!(e["type"], "redo");
    assert_eq!(e["deletedText"], "two ");
    assert_eq!(ids(&r), ["b1"]);
    assert_eq!(r.events().len(), 4, "insert, delete, undo, redo");
}

#[test]
fn a_paragraph_split_is_structure_not_a_large_text_event() {
    let (mut s, r) = fresh();
    // Capitalised already, so AutoCorrect leaves the sentence alone on Enter.
    run(&mut s, "text.insert", json!({"text": "Abcdef"}));
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [0], "off": 3}}));
    run(&mut s, "text.newParagraph", json!({}));
    assert_eq!(body(&s).lines().collect::<Vec<_>>(), ["Abc", "def"]);
    let e = last(&r);
    assert_eq!(e["type"], "structure_change");
    assert_eq!(e["origin"], "keyboard");
    assert_eq!(e["insertedText"], PARA_MARK.to_string(), "only the paragraph mark came");
    assert_eq!(e["deletedText"], "");
    assert_eq!(e["afterAnchor"]["blockId"], "b1");
    assert_eq!(e["afterAnchor"]["startOffset"], 3);
    // The first part keeps the paragraph's id; the new paragraph gets a new one.
    assert_eq!(ids(&r), ["b1", "b2"]);
    assert_eq!(e["ops"][0]["beforeBlocks"], json!([{"blockId": "b1", "len": 6}]));
    assert_eq!(e["ops"][0]["afterBlocks"], json!([{"blockId": "b1", "len": 3}, {"blockId": "b2", "len": 3}]));

    // Typing in the new paragraph anchors there.
    run(&mut s, "text.insert", json!({"text": "X"}));
    assert_eq!(last(&r)["afterAnchor"]["blockId"], "b2");
    assert_eq!(last(&r)["afterAnchor"]["startOffset"], 0);
    run(&mut s, "text.backspace", json!({}));

    // Backspace at the start of the second paragraph merges them: structure again.
    run(&mut s, "text.backspace", json!({}));
    assert_eq!(body(&s), "Abcdef");
    let e = last(&r);
    assert_eq!(e["type"], "structure_change");
    assert_eq!(e["deletedText"], PARA_MARK.to_string());
    assert_eq!(e["insertedText"], "");
    assert_eq!(ids(&r), ["b1"]);

    // Enter at the very start: the text keeps its id; the empty paragraph above is new.
    run(&mut s, "caret.set", json!({"pos": {"story": "body", "path": [0], "off": 0}}));
    run(&mut s, "text.newParagraph", json!({}));
    assert_eq!(last(&r)["type"], "structure_change");
    assert_eq!(ids(&r), ["b3", "b1"]);

    // Undo of a split is an undo event that takes the mark away and the ids back.
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(last(&r)["type"], "undo");
    assert_eq!(last(&r)["deletedText"], PARA_MARK.to_string());
    assert_eq!(ids(&r), ["b1"]);
}

#[test]
fn a_multi_paragraph_paste_is_one_event() {
    let (mut s, r) = fresh();
    run(&mut s, "text.insert", json!({"text": "Start "}));
    let gid = s.begin_gesture(Gesture::paste("edit.paste", None));
    run(&mut s, "edit.paste", json!({"text": "first line\nsecond line"}));
    s.end_gesture();
    let e = last(&r);
    assert_eq!(r.events().len(), 2);
    assert_eq!(e["type"], "paste");
    assert_eq!(e["gestureId"], json!(gid));
    let inserted = e["insertedText"].as_str().unwrap_or_default().replace(PARA_MARK, "\n");
    assert_eq!(inserted, "first line\nsecond line");
    assert_eq!(e["afterAnchor"]["blockId"], "b1");
    assert_eq!(e["afterAnchor"]["startOffset"], 6);
    // The range runs on through the paragraph mark into the next paragraph.
    assert_eq!(e["afterAnchor"]["endOffset"], 6 + "first line".len() + PARA_MARK.len_utf8() + "second line".len());
    assert_eq!(ids(&r), ["b1", "b2"]);
}

#[test]
fn a_command_that_changes_nothing_records_nothing() {
    let (mut s, r) = fresh();
    run(&mut s, "text.insert", json!({"text": "Hello"}));
    let n = r.events().len();
    run(&mut s, "caret.left", json!({}));
    run(&mut s, "caret.docStart", json!({}));
    run(&mut s, "select.all", json!({}));
    run(&mut s, "caret.docStart", json!({}));
    // Backspace at the start of the document changes nothing.
    run(&mut s, "text.backspace", json!({}));
    // Bold with no selection only sets the formatting waiting at the caret.
    run(&mut s, "format.bold", json!({}));
    // A failed command leaves the document as it was.
    assert!(s.run("styles.apply", &json!({"style": 5})).is_err());
    assert_eq!(r.events().len(), n);
}

#[test]
fn a_command_that_runs_another_is_one_event() {
    let (mut s, r) = fresh();
    run(&mut s, "text.insert", json!({"text": "Heading text"}));
    let n = r.events().len();
    // Apply Styles runs `para.style` as a command of its own.
    run(&mut s, "styles.apply", json!({"style": "Heading 1"}));
    let ev = r.events();
    assert_eq!(ev.len(), n + 1);
    let e = &ev[n];
    assert_eq!(e["type"], "format_change");
    assert_eq!(e["origin"], "editor_command");
    assert_eq!(e["ops"][0]["command"], "styles.apply");
    assert_eq!(e["insertedText"], "");
    assert_eq!(e["deletedText"], "");
}

#[test]
fn a_change_made_outside_run_is_recorded_as_unknown() {
    let (mut s, r) = fresh();
    run(&mut s, "text.insert", json!({"text": "Abc"}));
    // The front end edits the document directly, not through a command.
    s.doc = Document::from_text("Abc Zzz");
    run(&mut s, "caret.docEnd", json!({}));
    run(&mut s, "text.insert", json!({"text": "x"}));
    let ev = r.events();
    assert_eq!(ev.len(), 3);
    assert_eq!(ev[1]["origin"], "unknown");
    assert_eq!(ev[1]["insertedText"], " Zzz");
    assert_eq!(ev[2]["type"], "text_insert");
    assert_eq!(ev[2]["insertedText"], "x");
    assert_eq!(ev[2]["afterAnchor"]["blockId"], "b1");
    assert_eq!(ev[2]["afterAnchor"]["startOffset"], 7);
}

#[test]
fn a_new_document_is_an_import_with_fresh_ids() {
    let (mut s, r) = fresh();
    run(&mut s, "text.insert", json!({"text": "draft"}));
    run(&mut s, "file.new", json!({"template": "sample"}));
    let e = last(&r);
    assert_eq!(e["type"], "import_document");
    assert_eq!(e["origin"], "import");
    assert_eq!(e["ops"][0]["op"], "replaceDocument");
    assert!(!ids(&r).contains(&"b1".to_string()), "the new document's paragraphs get new ids");
    // Edits after it anchor on the new ids.
    run(&mut s, "text.insert", json!({"text": "x"}));
    assert_eq!(last(&r)["type"], "text_insert");
    assert!(ids(&r).contains(&last(&r)["afterAnchor"]["blockId"].as_str().unwrap_or_default().to_string()));
}

#[test]
fn the_event_chain_links_and_verifies() {
    let (mut s, r) = fresh();
    type_keys(&mut s, "ab");
    run(&mut s, "text.newParagraph", json!({}));
    let ev = r.events();
    let mut prior = sha256_hex(DOMAIN_GENESIS, ATTEMPT.as_bytes());
    for e in &ev {
        assert_eq!(e["priorEventDigest"], prior.as_str());
        let mut body = e.clone();
        let digest = body.as_object_mut().and_then(|m| m.remove("eventDigest")).unwrap_or(Value::Null);
        assert_eq!(digest, sha256_hex(DOMAIN_EVENT, canonical_json(&body).as_bytes()).as_str());
        prior = digest.as_str().unwrap_or_default().to_string();
    }
    // The state digest is the document's: the same text gives the same digest.
    let d = ev.last().map(|e| e["documentStateDigest"].clone()).unwrap_or(Value::Null);
    assert_eq!(d, r.state_digest(&s.doc).as_str());
    let first = &ev[0]["documentStateDigest"];
    assert_ne!(first, &d);
}

/// Checks a value against the schema's `$defs/event` (required keys, no extra keys, enums,
/// digests, anchors): enough to hold the recorder to the schema without a validator crate.
fn check_event(schema: &Value, e: &Value) {
    let def = &schema["$defs"]["event"];
    let props = def["properties"].as_object().cloned().unwrap_or_default();
    let obj = e.as_object().cloned().unwrap_or_default();
    for k in def["required"].as_array().cloned().unwrap_or_default() {
        assert!(obj.contains_key(k.as_str().unwrap_or_default()), "missing {k} in {e}");
    }
    for (k, v) in &obj {
        let p = props.get(k).unwrap_or_else(|| panic!("`{k}` is not in the schema"));
        if let Some(allowed) = p["enum"].as_array() {
            assert!(allowed.contains(v), "{k} = {v} not in {allowed:?}");
        }
        if p["pattern"] == "^[0-9a-f]{64}$" {
            let s = v.as_str().unwrap_or_default();
            assert!(s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{k}");
        }
    }
    assert!(e["seq"].as_u64().is_some_and(|n| n >= 1));
    assert!(e["elapsedMonotonicMs"].is_number());
    assert!(e["ops"].as_array().is_some_and(|o| o.iter().all(Value::is_object)));
    let id = e["eventId"].as_str().unwrap_or_default();
    assert!(id.len() == 36 && id.chars().nth(14) == Some('4'), "uuid v4: {id}");
    let anchor = &schema["$defs"]["anchor"];
    let aprops = anchor["properties"].as_object().cloned().unwrap_or_default();
    for key in ["beforeAnchor", "afterAnchor"] {
        let Some(a) = e[key].as_object() else {
            assert!(e[key].is_null());
            continue;
        };
        for k in anchor["required"].as_array().cloned().unwrap_or_default() {
            assert!(a.contains_key(k.as_str().unwrap_or_default()), "{key} lacks {k}");
        }
        for k in a.keys() {
            assert!(aprops.contains_key(k), "{key}.{k} is not in the schema");
        }
        assert!(a["startOffset"].as_u64().is_some() && a["endOffset"].as_u64().is_some());
        assert!(a["blockId"].is_string());
    }
}

#[test]
fn events_match_the_evidence_schema() {
    let schema: Value = serde_json::from_str(include_str!("../../../../../schemas/evidence-v1.schema.json")).unwrap_or(Value::Null);
    assert!(schema.is_object(), "schema parses");
    let (mut s, r) = fresh();
    type_keys(&mut s, "Hi");
    run(&mut s, "text.newParagraph", json!({}));
    s.begin_gesture(Gesture::paste("edit.paste", Some(vec!["text/plain".into(), "text/html".into()])));
    run(&mut s, "edit.paste", json!({"text": text_185()}));
    s.end_gesture();
    run(&mut s, "select.all", json!({}));
    run(&mut s, "format.bold", json!({}));
    run(&mut s, "edit.cut", json!({}));
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.redo", json!({}));
    let ev = r.events();
    let types: Vec<&str> = ev.iter().filter_map(|e| e["type"].as_str()).collect();
    assert_eq!(types, ["text_insert", "text_insert", "structure_change", "paste", "format_change", "cut", "undo", "redo"]);
    for e in &ev {
        check_event(&schema, e);
    }
    // For a full JSON Schema validator outside Rust: `INKWITNESS_DUMP_EVENTS=<file>`.
    if let Ok(path) = std::env::var("INKWITNESS_DUMP_EVENTS") {
        std::fs::write(path, serde_json::to_string_pretty(&ev).unwrap()).unwrap();
    }
}

#[test]
fn hostile_params_with_a_recorder_never_panic() {
    let reg = wordcraft_engine::cmd::registry();
    let junk = [json!(null), json!({}), json!({"text": 5, "pos": {"block": 9999, "off": 99999}, "style": [], "count": 1e9})];
    for spec in reg.all() {
        if spec.id.starts_with("file.") || spec.id == "insert.picture" || spec.id == "insert.textFromFile" {
            continue;
        }
        for j in &junk {
            let mut s = Session::new(wordcraft_engine::sample::sample_document());
            let r = Recorder::attach(&mut s, Box::new(TestEnv { ms: 0, ids: 0 }), config());
            s.begin_gesture(Gesture::paste(spec.id, None));
            let _ = s.run(spec.id, j);
            s.end_gesture();
            for e in r.events() {
                assert!(e["seq"].as_u64().is_some(), "{}", spec.id);
            }
        }
    }
}
