//! What one step of the review is (FAL-678). The writer records typing as one event per word, so
//! ← and → step a word at a time. A file from a 1.0 writer has one event per key press: its
//! letters are grouped into words here, on screen only, by the same rules the writer merges by
//! ([`wordcraft_inkwitness::MERGE_PAUSE_MS`] and friends). The file isn't changed, and every
//! event keeps its own number: a step ends at an event, and Seek goes there.

use serde_json::Value;
use wordcraft_inkwitness::{MERGE_MAX_KEYS, MERGE_PAUSE_MS, PARA_MARK, ends_word};

/// What a typing event of a per-letter file did: enough to tell whether the next one continues it.
struct Key {
    kind: u8,
    story: String,
    block: String,
    offset: u64,
    len: u64,
    ms: u64,
    /// An inserted character's text (empty for a deletion).
    typed: String,
}

/// The key press an old event records: a typed character, a Backspace or a Delete in one
/// paragraph. A merged event (it has `keyCount`) is a step of its own, so it is none.
fn key(e: &Value) -> Option<Key> {
    if e.get("keyCount").is_some() || e.get("origin")?.as_str()? != "keyboard" {
        return None;
    }
    let ops = e.get("ops")?.as_array()?;
    let [op] = ops.as_slice() else { return None };
    let command = op.get("command")?.as_str()?;
    let kind = match command {
        "text.insert" => 0,
        "text.backspace" => 1,
        "text.delete" => 2,
        _ => return None,
    };
    let text = op.get(if kind == 0 { "inserted" } else { "deleted" })?.as_str()?;
    let other = op.get(if kind == 0 { "deleted" } else { "inserted" })?.as_str()?;
    let window =
        |k: &str| op.get(k).and_then(Value::as_array).filter(|w| w.len() == 1).and_then(|w| w.first()?.get("blockId")?.as_str().map(str::to_string));
    if op.get("op")?.as_str()? != "replace" || text.is_empty() || !other.is_empty() || text.contains(PARA_MARK) {
        return None;
    }
    Some(Key {
        kind,
        story: op.get("story")?.as_str()?.to_string(),
        block: window("beforeBlocks").filter(|b| window("afterBlocks").as_ref() == Some(b))?,
        offset: op.get("offset")?.as_u64()?,
        len: text.len() as u64,
        ms: e.get("elapsedMonotonicMs").and_then(Value::as_u64).unwrap_or(0),
        typed: if kind == 0 { text.to_string() } else { String::new() },
    })
}

/// Whether `next` continues the typing `last` ended, the way the writer joins key presses.
fn continues(last: &Key, group: &str, next: &Key) -> bool {
    last.kind == next.kind
        && last.story == next.story
        && last.block == next.block
        && !(last.kind == 0 && ends_word(group))
        && next.ms.saturating_sub(last.ms) <= MERGE_PAUSE_MS
        && match last.kind {
            0 => next.offset == last.offset + last.len,
            1 => next.offset + next.len == last.offset,
            _ => next.offset == last.offset,
        }
}

/// The last event of each step, in order, over the first `upto` events: a merged event, or any
/// event that isn't a key press, is a step of its own; the key presses of an old file are grouped
/// into words. The last event always ends a step.
pub fn ends(events: &[Value], upto: u64) -> Vec<u64> {
    let events = &events[..events.len().min(usize::try_from(upto).unwrap_or(usize::MAX))];
    let keys: Vec<Option<Key>> = events.iter().map(key).collect();
    let mut out = Vec::new();
    let (mut run, mut group) = (0u64, String::new());
    for (i, k) in keys.iter().enumerate() {
        run = if k.is_some() { run + 1 } else { 0 };
        if let Some(k) = k {
            group.push_str(&k.typed);
        }
        let joins = match (k, keys.get(i + 1)) {
            (Some(a), Some(Some(b))) => run < MERGE_MAX_KEYS && continues(a, &group, b),
            _ => false,
        };
        if !joins {
            out.push(i as u64 + 1);
            (run, group) = (0, String::new());
        }
    }
    out
}

/// Typing speed of a merged event, in keys a minute, from its `keyCount` and its start and end
/// times. `None` for a single key press (no time between keys) and for events without the fields.
pub fn keys_per_minute(e: &Value) -> Option<u64> {
    let keys = e.get("keyCount")?.as_u64().filter(|k| *k >= 2)?;
    let start = e.get("elapsedMonotonicMs")?.as_u64()?;
    let span = e.get("endMonotonicMs")?.as_u64()?.checked_sub(start).filter(|s| *s > 0)?;
    // Between the first and the last key there are one fewer gaps than keys.
    Some((keys - 1) * 60_000 / span)
}

/// "11 keys, 300 keys a minute" for a merged typing event, from `keyCount` and its times.
pub fn typing_line(e: &Value) -> Option<String> {
    let keys = e.get("keyCount")?.as_u64()?;
    let n = format!("{keys} {}", if keys == 1 { "key" } else { "keys" });
    Some(match keys_per_minute(e) {
        Some(rate) => format!("{n}, {rate} keys a minute"),
        None => n,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn press(seq: u64, ms: u64, offset: u64, text: &str) -> Value {
        json!({"seq": seq, "origin": "keyboard", "elapsedMonotonicMs": ms,
            "ops": [{"op": "replace", "command": "text.insert", "story": "body", "beforeBlocks": [{"blockId": "b1", "len": offset}],
                "afterBlocks": [{"blockId": "b1", "len": offset + 1}], "offset": offset, "deleted": "", "inserted": text}]})
    }

    fn word(start: u64, ms: u64, text: &str) -> Vec<Value> {
        text.char_indices().map(|(i, c)| press(start + i as u64, ms + 100 * i as u64, (i as u64) + (start - 1), &c.to_string())).collect()
    }

    #[test]
    fn letters_of_an_old_file_group_into_words_ending_at_a_space() {
        let mut ev = word(1, 0, "the ");
        ev.extend(word(5, 400, "cat"));
        assert_eq!(ends(&ev, 7), [4, 7]);
        // Replay stops short of the last events: the steps stop there too.
        assert_eq!(ends(&ev, 6), [4, 6]);
    }

    #[test]
    fn a_pause_over_two_seconds_or_a_jump_ends_a_step() {
        let mut ev = word(1, 0, "ab");
        ev.extend(word(3, 2_000, "cd"));
        ev.extend(word(5, 6_000, "ef"));
        assert_eq!(ends(&ev, 6), [4, 6], "1.9 s joins, 4 s doesn't");
        let moved = vec![press(1, 0, 0, "a"), press(2, 100, 5, "b")];
        assert_eq!(ends(&moved, 2), [1, 2], "typing somewhere else is a new step");
    }

    #[test]
    fn merged_events_and_other_commands_are_steps_of_their_own() {
        let mut merged = word(1, 0, "ab");
        merged.iter_mut().for_each(|e| e["keyCount"] = json!(1));
        assert_eq!(ends(&merged, 2), [1, 2]);
        let other = vec![json!({"seq": 1, "type": "paste"}), json!({"seq": 2, "type": "undo"})];
        assert_eq!(ends(&other, 2), [1, 2]);
        assert!(ends(&[], 0).is_empty());
    }

    #[test]
    fn a_long_run_is_cut_like_the_writer_cuts_it() {
        let ev = word(1, 0, &"x".repeat(70));
        assert_eq!(ends(&ev, 70), [32, 64, 70]);
    }

    #[test]
    fn typing_speed_comes_from_the_key_count_and_the_times() {
        let e = json!({"keyCount": 11, "elapsedMonotonicMs": 1_000, "endMonotonicMs": 3_000});
        assert_eq!(keys_per_minute(&e), Some(300), "ten gaps in two seconds: five keys a second");
        assert_eq!(keys_per_minute(&json!({"keyCount": 1, "elapsedMonotonicMs": 1, "endMonotonicMs": 1})), None);
        assert_eq!(keys_per_minute(&json!({"elapsedMonotonicMs": 1})), None);
        assert_eq!(typing_line(&e).as_deref(), Some("11 keys, 300 keys a minute"));
        assert_eq!(typing_line(&json!({"keyCount": 1})).as_deref(), Some("1 key"));
        assert_eq!(typing_line(&json!({})), None);
    }
}
