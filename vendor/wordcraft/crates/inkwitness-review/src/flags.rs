//! The review's flags (FAL-519, from FAL-477): the moments in a history a teacher should look at,
//! computed in one place with the rule IDs of `docs/DIAGNOSTICS.md`. The timeline's pins, the
//! rail's flag cards, Previous / Next flag and Pause at flags all read [`compute`].
//!
//! A flag says what happened ("Paste · 185 characters"), never that a student cheated: no
//! probability, no score, no "misconduct" (`AGENTS.md`). The AI-style phrase rule quotes the exact
//! wording it matched, as a lead for a conversation and nothing more (`docs/REPLAY_UI.md`).

use serde_json::Value;

use crate::card::count;
use crate::review::{Consistency, Loaded};

/// A paste this long (in characters, paragraph marks not counted) is a large paste.
pub const LARGE_PASTE_CHARS: usize = 150;
/// A paste with this many words is a large paste.
pub const LARGE_PASTE_WORDS: usize = 30;
/// Text this long added in one committed edit, without a paste, is a large insertion.
pub const LARGE_INSERT_CHARS: usize = 150;
/// Text this long removed in one committed edit is a large deletion.
pub const LARGE_DELETE_CHARS: usize = 150;

/// Wording that chatbot answers wrap around their text (`wa.text.template-marker.v1`). Matched
/// case-insensitively, with typographic apostrophes read as plain ones, and only in pasted,
/// imported or large inserted passages: typed text arrives a character at a time.
pub const TEMPLATE_MARKERS: &[&str] = &[
    "as an ai language model",
    "as an ai model",
    "as a large language model",
    "i'm just an ai",
    "i am an ai",
    "as of my last knowledge update",
    "as of my knowledge cutoff",
    "my knowledge cutoff",
    "i hope this helps",
    "certainly! here",
    "sure! here",
    "here's a revised version",
    "here is a revised version",
    "here's an essay",
    "here is an essay",
    "regenerate response",
    "let me know if you'd like",
    "let me know if you would like",
    "feel free to ask if",
];

/// What a flag is about. Each kind has its own pin shape, icon and words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FlagKind {
    /// `wa.clipboard.real-paste.v1`: any paste.
    Paste,
    /// `wa.clipboard.large-paste.v1`.
    LargePaste,
    /// `wa.edit.large-insert.v1`: a large insertion with no paste gesture.
    LargeInsert,
    /// `wa.edit.large-delete.v1`.
    LargeDelete,
    /// `wa.import.unobserved.v1`: text that was already there when the document was opened.
    Imported,
    /// `wa.text.template-marker.v1`: chatbot wording in a pasted, imported or inserted passage.
    AiPhrase,
    /// Where a corrupt history stops replaying.
    HistoryStops,
    /// `wa.outside.large-addition.v1`: a long passage in a file changed outside InkWitness
    /// ([`crate::outside`]), flagged the way a large paste is.
    OutsideAddition,
}

impl FlagKind {
    /// The rule ID and version (`docs/DIAGNOSTICS.md`).
    pub fn rule(self) -> &'static str {
        match self {
            FlagKind::Paste => "wa.clipboard.real-paste.v1",
            FlagKind::LargePaste => "wa.clipboard.large-paste.v1",
            FlagKind::LargeInsert => "wa.edit.large-insert.v1",
            FlagKind::LargeDelete => "wa.edit.large-delete.v1",
            FlagKind::Imported => "wa.import.unobserved.v1",
            FlagKind::AiPhrase => "wa.text.template-marker.v1",
            FlagKind::HistoryStops => "wa.history.stops.v1",
            FlagKind::OutsideAddition => "wa.outside.large-addition.v1",
        }
    }

    /// The chip's words.
    pub fn name(self) -> &'static str {
        match self {
            FlagKind::Paste => "Paste",
            FlagKind::LargePaste => "Large paste",
            FlagKind::LargeInsert => "Large insertion",
            FlagKind::LargeDelete => "Large deletion",
            FlagKind::Imported => "Text already there",
            FlagKind::AiPhrase => "AI-style phrase",
            FlagKind::HistoryStops => "History stops",
            FlagKind::OutsideAddition => "Large addition outside",
        }
    }

    /// The chip's [`wordcraft_ui_egui::icons`] name.
    pub fn icon(self) -> &'static str {
        match self {
            FlagKind::Paste | FlagKind::LargePaste | FlagKind::OutsideAddition => "paste",
            FlagKind::LargeInsert => "addText",
            FlagKind::LargeDelete => "eraser",
            FlagKind::Imported => "open",
            FlagKind::AiPhrase => "search",
            FlagKind::HistoryStops => "close",
        }
    }

    /// A strong flag gets a bigger, filled pin; a plain paste a small one.
    pub fn strong(self) -> bool {
        !matches!(self, FlagKind::Paste)
    }

    /// What the rule found, and what it can't tell (`docs/DIAGNOSTICS.md`, "Output").
    pub fn meaning(self) -> &'static str {
        match self {
            FlagKind::Paste => "Text was pasted here. A browser can't tell where it came from.",
            FlagKind::LargePaste => "A long passage was pasted in one go. Worth asking where it came from.",
            FlagKind::LargeInsert => {
                "A long passage arrived in one edit without a paste. How it was entered can't be told: dictation, autocomplete or a drop look the same."
            }
            FlagKind::LargeDelete => "A long passage was removed in one edit. The replay still has the original.",
            FlagKind::Imported => "This text was in the document when it was opened, so how it was written wasn't observed.",
            FlagKind::AiPhrase => {
                "The passage contains wording that chatbot answers often use. A lead for a conversation, not a finding: people quote and reuse this wording for many reasons."
            }
            FlagKind::HistoryStops => "The recorded history stops replaying here. Later events can't be shown.",
            FlagKind::OutsideAddition => {
                "A long passage is in the file that the history doesn't end with. It was added where InkWitness wasn't recording, so when it was added and where it came from are both unknown. Worth asking."
            }
        }
    }
}

/// One flagged moment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flag {
    /// The event it is about (pins and Go to seek to exactly this event).
    pub seq: u64,
    pub kind: FlagKind,
    /// "Paste · 185 characters".
    pub label: String,
    /// The exact wording an [`FlagKind::AiPhrase`] matched, as it appears in the passage.
    pub matched: Option<String>,
}

/// Characters and words in `t`, paragraph marks not counted.
pub fn measure(t: &str) -> (usize, usize) {
    let chars = t.chars().filter(|c| *c != wordcraft_inkwitness::PARA_MARK).count();
    let words = t.split(|c: char| c.is_whitespace() || c == wordcraft_inkwitness::PARA_MARK).filter(|w| !w.is_empty()).count();
    (chars, words)
}

/// The first template marker in `text`, as it is written there.
pub fn template_marker(text: &str) -> Option<String> {
    let folded: String = text.chars().map(|c| if c == '\u{2019}' || c == '\u{2018}' { '\'' } else { c }).collect();
    let lower = folded.to_lowercase();
    // Lower-casing can change byte lengths; match on chars so the quote is cut where it matched.
    let (lc, oc): (Vec<char>, Vec<char>) = (lower.chars().collect(), text.chars().collect());
    if lc.len() != oc.len() {
        return TEMPLATE_MARKERS.iter().find(|m| lower.contains(*m)).map(|m| (*m).to_string());
    }
    TEMPLATE_MARKERS
        .iter()
        .filter_map(|m| {
            let mc: Vec<char> = m.chars().collect();
            let at = lc.windows(mc.len()).position(|w| w == mc.as_slice())?;
            Some((at, oc.get(at..at + mc.len())?.iter().collect::<String>()))
        })
        .min_by_key(|(at, _)| *at)
        .map(|(_, s)| s)
}

/// The flags of event `e`, strongest first.
pub fn of_event(e: &Value) -> Vec<Flag> {
    let Some(seq) = e.get("seq").and_then(Value::as_u64) else { return Vec::new() };
    let s = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or_default();
    let ty = s("type");
    let (ic, iw) = measure(s("insertedText"));
    let (dc, _) = measure(s("deletedText"));
    let chars = |n: usize| count(n, "character", "characters");
    let mut out = Vec::new();
    let flag = |kind: FlagKind, label: String| Flag { seq, kind, label, matched: None };
    let pasted = ty == "paste" || !s("pasteGestureId").is_empty();
    let passage = if ty == "import_document" {
        // A New, or a file with no text, put nothing there.
        if ic > 0 {
            out.push(flag(FlagKind::Imported, format!("Text already there · {}", chars(ic))));
        }
        ic > 0
    } else if pasted {
        let large = ic >= LARGE_PASTE_CHARS || iw >= LARGE_PASTE_WORDS;
        out.push(flag(
            if large { FlagKind::LargePaste } else { FlagKind::Paste },
            format!("{} · {}", if large { "Large paste" } else { "Paste" }, chars(ic)),
        ));
        true
    } else if ic >= LARGE_INSERT_CHARS {
        out.push(flag(FlagKind::LargeInsert, format!("Large insertion · {}", chars(ic))));
        true
    } else {
        false
    };
    if passage && let Some(m) = template_marker(s("insertedText")) {
        out.insert(0, Flag { seq, kind: FlagKind::AiPhrase, label: format!("AI-style phrase · “{m}”"), matched: Some(m) });
    }
    if dc >= LARGE_DELETE_CHARS {
        out.push(flag(FlagKind::LargeDelete, format!("Large deletion · {}", chars(dc))));
    }
    out
}

/// Every flag in the history a reviewer can seek to, in event order (an event with two flags
/// gives two, strongest first), and the point where a corrupt history stops.
pub fn compute(l: &Loaded) -> Vec<Flag> {
    let mut out: Vec<Flag> = l
        .replay
        .events()
        .iter()
        .filter(|e| e.get("seq").and_then(Value::as_u64).is_some_and(|s| s <= l.replayable))
        .flat_map(|e| if e.get("type").and_then(Value::as_str) == Some("import_document") { of_event(&opened(l, e)) } else { of_event(e) })
        .collect();
    if let Consistency::Corrupt(_) = l.consistency {
        out.push(Flag { seq: l.replayable, kind: FlagKind::HistoryStops, label: "History stops replaying".into(), matched: None });
    }
    out
}

/// An `import_document` event with the text of the document it opened as its `insertedText`:
/// the event names the new paragraphs but doesn't carry their text, so it is read from the
/// state the event left.
fn opened(l: &Loaded, e: &Value) -> Value {
    let text = e
        .get("seq")
        .and_then(Value::as_u64)
        .and_then(|s| l.replay.state_at(s).ok())
        .map(|st| st.doc.plain_text(wordcraft_doc::StoryRef::Body).replace('\n', &wordcraft_inkwitness::PARA_MARK.to_string()))
        .unwrap_or_default();
    let mut e = e.clone();
    if let Some(m) = e.as_object_mut() {
        m.insert("insertedText".into(), Value::String(text));
    }
    e
}

/// The `seq` of each flagged event, once each, in order.
pub fn moments(flags: &[Flag]) -> Vec<u64> {
    let mut v: Vec<u64> = flags.iter().map(|f| f.seq).collect();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ev(seq: u64, ty: &str, ins: &str, del: &str) -> Value {
        json!({"seq": seq, "type": ty, "insertedText": ins, "deletedText": del})
    }

    #[test]
    fn a_short_paste_is_a_plain_paste_and_a_long_one_is_large() {
        let f = of_event(&ev(3, "paste", "a quote", ""));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, FlagKind::Paste);
        assert_eq!(f[0].label, "Paste · 7 characters");
        let long = "x".repeat(LARGE_PASTE_CHARS);
        assert_eq!(of_event(&ev(3, "paste", &long, ""))[0].kind, FlagKind::LargePaste);
        // 30 short words is large too.
        let words = "a ".repeat(LARGE_PASTE_WORDS);
        assert_eq!(of_event(&ev(3, "paste", &words, ""))[0].kind, FlagKind::LargePaste);
        // Paragraph marks don't count as characters.
        let marks = format!("{}{}", "x".repeat(LARGE_PASTE_CHARS - 1), wordcraft_inkwitness::PARA_MARK);
        assert_eq!(of_event(&ev(3, "paste", &marks, ""))[0].kind, FlagKind::Paste);
    }

    #[test]
    fn typing_is_not_flagged_but_a_large_insertion_is() {
        assert!(of_event(&ev(1, "text_insert", "a", "")).is_empty());
        let long = "y".repeat(LARGE_INSERT_CHARS);
        let f = of_event(&ev(1, "text_insert", &long, ""));
        assert_eq!(f[0].kind, FlagKind::LargeInsert);
        assert_eq!(of_event(&ev(1, "text_delete", "", &long))[0].kind, FlagKind::LargeDelete);
        assert_eq!(of_event(&ev(1, "import_document", "starting text", ""))[0].kind, FlagKind::Imported);
        // A blank New: nothing was there.
        assert!(of_event(&ev(1, "import_document", "", "")).is_empty());
    }

    #[test]
    fn chatbot_wording_in_a_paste_is_quoted_as_it_was_written() {
        let f = of_event(&ev(5, "paste", "Certainly! Here is the paragraph. I hope this helps.", ""));
        assert_eq!(f[0].kind, FlagKind::AiPhrase);
        assert_eq!(f[0].matched.as_deref(), Some("Certainly! Here"));
        assert_eq!(f[1].kind, FlagKind::Paste);
        assert_eq!(template_marker("As an AI language model, I can’t"), Some("As an AI language model".into()));
        assert_eq!(template_marker("Let me know if you’d like more."), Some("Let me know if you’d like".into()));
        // Typed one character at a time, wording is never matched.
        assert!(of_event(&ev(5, "text_insert", "I hope this helps", "")).iter().all(|f| f.kind != FlagKind::AiPhrase));
        assert_eq!(template_marker("An essay about artificial intelligence."), None);
    }

    #[test]
    fn labels_never_accuse() {
        let all = [
            FlagKind::Paste,
            FlagKind::LargePaste,
            FlagKind::LargeInsert,
            FlagKind::LargeDelete,
            FlagKind::Imported,
            FlagKind::AiPhrase,
            FlagKind::HistoryStops,
            FlagKind::OutsideAddition,
        ];
        for k in all {
            for text in [k.name(), k.meaning()] {
                let low = text.to_lowercase();
                for banned in ["suspicious", "misconduct", "cheat", "probability", "detected", "score", "plagiar"] {
                    assert!(!low.contains(banned), "{k:?} says {banned:?}: {text}");
                }
            }
        }
    }
}
