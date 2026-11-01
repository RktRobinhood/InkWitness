//! Findings (FAL-665): a flag as the reviewer reads it. A flag ([`crate::flags`]) says where
//! something happened; a [`Finding`] says what was observed, what supports it, what else could
//! explain it, what to check next, what the check does not prove, and what the teacher decided.
//!
//! # The contract
//!
//! The reader (the rail, the review copy) draws a [`Finding`] and nothing else, so a later check
//! (similarity, references, any model) adds a rule and fills a `Finding` without changing it:
//!
//! | Field | Meaning |
//! |---|---|
//! | `rule` | The rule id and version, `wa.<area>.<name>.v<N>` (`docs/DIAGNOSTICS.md`). |
//! | `method` | The version of this wording and shape ([`METHOD_VERSION`]). An old report keeps its meaning when it changes. |
//! | `observed` | One sentence of what happened, as recorded. |
//! | `record` | The supporting record: the event, when, how much. |
//! | `alternatives` | Plausible other explanations, at least two, neutral and not ranked. |
//! | `next_check` | The verification action: something the teacher can do, at least one. |
//! | `not_proven` | What the finding does not prove. |
//! | `resolution` | The teacher's decision ([`Resolution`]), kept with their comments ([`crate::comments::TeacherComment`]). |
//!
//! A finding has no score, no total and no ranking: nothing here is a number the teacher could add
//! up, and a test holds the wording to that.

use crate::flags::{Flag, FlagKind};

/// The version of the findings' shape and wording. Raise it when an alternative, a check or a
/// limit changes meaning, so a report made earlier still reads as it did.
pub const METHOD_VERSION: &str = "findings.v1";

/// What the teacher decided about a finding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Resolution {
    /// Not looked at, or still being looked at.
    #[default]
    Open,
    /// The student, or the record, explained it.
    Explained,
    /// The teacher wants to talk with the student first.
    NeedsConversation,
    /// Nothing to follow up.
    Dismissed,
}

impl Resolution {
    pub const ALL: [Resolution; 4] = [Resolution::Open, Resolution::Explained, Resolution::NeedsConversation, Resolution::Dismissed];

    pub fn name(self) -> &'static str {
        match self {
            Resolution::Open => "Open",
            Resolution::Explained => "Explained",
            Resolution::NeedsConversation => "Needs conversation",
            Resolution::Dismissed => "Dismissed",
        }
    }
}

/// One finding. See the module's contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub rule: String,
    pub method: &'static str,
    pub seq: u64,
    pub kind: FlagKind,
    pub observed: String,
    pub record: String,
    pub alternatives: Vec<&'static str>,
    pub next_check: Vec<&'static str>,
    pub not_proven: &'static str,
    pub resolution: Resolution,
}

/// The ways a passage that arrived whole can have come to be there.
const PASSAGE: [&str; 5] = [
    "A quoted source the student meant to cite.",
    "The student's own earlier draft, written elsewhere and brought in.",
    "An assistive workflow: dictation, speech to text, or an accessibility tool.",
    "Text from outside the student's own writing that wasn't declared.",
    "Another explanation the record can't show.",
];

/// What else could explain a flag of `kind` (at least two, neutral).
pub fn alternatives(kind: FlagKind) -> Vec<&'static str> {
    match kind {
        FlagKind::Paste | FlagKind::LargePaste => PASSAGE.to_vec(),
        FlagKind::LargeInsert => vec![
            "Dictation or speech to text.",
            "Autocomplete, a text expander or a drag and drop.",
            "The student's own earlier draft, brought in.",
            "Text from outside the student's own writing that wasn't declared.",
            "Another explanation the record can't show.",
        ],
        FlagKind::LargeDelete => vec![
            "The student cut their own draft to rewrite it.",
            "A section was moved: cut here and pasted elsewhere.",
            "A mistaken selection, undone or kept.",
            "Another explanation the record can't show.",
        ],
        FlagKind::Imported => vec![
            "The student's own earlier work, written before this file was opened.",
            "A teacher's template or a starting text the task provided.",
            "Text from outside the student's own writing that wasn't declared.",
            "Another explanation the record can't show.",
        ],
        FlagKind::AiPhrase => vec![
            "A quotation from a source about the topic, cited or about to be.",
            "The student discussing such tools in their own words.",
            "The student's own earlier draft, or wording they reused.",
            "An assistive workflow, or text from outside that wasn't declared.",
            "Another explanation the record can't show.",
        ],
        FlagKind::HistoryStops => vec![
            "The file was changed by a program that doesn't record, such as another editor.",
            "The history was damaged when the file was copied, synced or emailed.",
            "Another explanation the record can't show.",
        ],
        FlagKind::OutsideAddition => vec![
            "Edits made in Word or another editor that doesn't record.",
            "Text copied in from a document the student wrote earlier.",
            "A tool or person other than the student added it, or the text is from an undeclared source.",
            "Another explanation the record can't show.",
        ],
    }
}

/// What the teacher can check next for a flag of `kind` (at least one).
pub fn next_checks(kind: FlagKind) -> Vec<&'static str> {
    match kind {
        FlagKind::Paste | FlagKind::LargePaste => vec![
            "Read the passage in the replay and ask the student where it came from.",
            "Check whether the assignment asked for sources, and whether this passage cites one.",
        ],
        FlagKind::LargeInsert => vec![
            "Ask the student how this part was entered and whether they used dictation or another tool.",
            "Compare its style with the student's typed text before and after it.",
        ],
        FlagKind::LargeDelete => vec!["Step back through the replay: the removed text is still there to read, and ask what changed."],
        FlagKind::Imported => vec!["Ask when and where the student wrote this text, and whether an earlier version exists."],
        FlagKind::AiPhrase => vec![
            "Read the passage around the wording and ask the student about it.",
            "Check whether the wording is a quotation, and whether the student can explain the passage.",
        ],
        FlagKind::HistoryStops => vec!["Ask the student which program saved the file last, and compare with an earlier copy if one exists."],
        FlagKind::OutsideAddition => vec![
            "Ask the student when and where the added text was written.",
            "Ask for the version of the file from before the change, if the student has one.",
        ],
    }
}

/// What a finding of `kind` does not prove.
pub fn not_proven(kind: FlagKind) -> &'static str {
    match kind {
        FlagKind::Paste | FlagKind::LargePaste => {
            "It doesn't show where the text came from, who wrote it, or whether it was allowed. A browser can't tell the source of pasted text."
        }
        FlagKind::LargeInsert => "It doesn't show how the text was entered, or who wrote it.",
        FlagKind::LargeDelete => "It doesn't show why the text was removed.",
        FlagKind::Imported => "It doesn't show how, when or by whom the text was written: nothing about it was observed.",
        FlagKind::AiPhrase => "Matching wording is not evidence of how the passage was written. People quote and reuse this wording for many reasons.",
        FlagKind::HistoryStops => "It doesn't show what happened to the file, or that anyone changed it on purpose.",
        FlagKind::OutsideAddition => "It doesn't show when the text was added, in which program, or by whom: InkWitness wasn't recording.",
    }
}

impl Finding {
    /// The finding for `flag`, not yet resolved. `record` is the supporting record as the rail
    /// shows it (the event's time and size), made by the caller.
    pub fn of(flag: &Flag, record: String) -> Finding {
        let mut observed = flag.label.clone();
        if let Some(m) = &flag.matched {
            observed.push_str(&format!(" (matched wording: “{m}”)"));
        }
        Finding {
            rule: flag.kind.rule().to_string(),
            method: METHOD_VERSION,
            seq: flag.seq,
            kind: flag.kind,
            observed,
            record,
            alternatives: alternatives(flag.kind),
            next_check: next_checks(flag.kind),
            not_proven: not_proven(flag.kind),
            resolution: Resolution::Open,
        }
    }

    /// Every sentence a finding says, for the wording tests.
    pub fn words(&self) -> Vec<String> {
        let mut v = vec![self.observed.clone(), self.record.clone(), self.not_proven.to_string(), self.method.to_string()];
        v.extend(self.alternatives.iter().map(|s| s.to_string()));
        v.extend(self.next_check.iter().map(|s| s.to_string()));
        v
    }
}

/// The text a resolution is given in the review copy (a Word comment) and the in-page list.
pub fn resolution_text(kind: FlagKind, res: Resolution, note: &str) -> String {
    let head = format!("Finding · {} · {}: {}.", kind.name(), METHOD_VERSION, res.name());
    if note.trim().is_empty() { head } else { format!("{head}\n{}", note.trim()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const ALL: [FlagKind; 8] = [
        FlagKind::Paste,
        FlagKind::LargePaste,
        FlagKind::LargeInsert,
        FlagKind::LargeDelete,
        FlagKind::Imported,
        FlagKind::AiPhrase,
        FlagKind::HistoryStops,
        FlagKind::OutsideAddition,
    ];

    fn finding(kind: FlagKind) -> Finding {
        Finding::of(&Flag { seq: 4, kind, label: format!("{} · 185 characters", kind.name()), matched: None }, "Event 4 · 1:05 into the session".into())
    }

    #[test]
    fn every_kind_has_two_alternatives_a_next_check_and_its_limits() {
        for k in ALL {
            let f = finding(k);
            assert!(f.alternatives.len() >= 2, "{k:?}");
            assert!(!f.next_check.is_empty(), "{k:?}");
            assert!(!f.not_proven.is_empty(), "{k:?}");
            assert!(f.method.starts_with("findings.v"));
            assert_eq!(f.rule, k.rule());
            assert_eq!(f.resolution, Resolution::Open);
        }
    }

    #[test]
    fn a_pasted_block_lists_the_five_explanations() {
        let alts = finding(FlagKind::Paste).alternatives.join(" ").to_lowercase();
        for part in ["quoted source", "earlier draft", "assistive workflow", "outside", "another explanation"] {
            assert!(alts.contains(part), "{part}");
        }
    }

    #[test]
    fn no_finding_carries_a_score_a_total_or_a_risk_figure() {
        for k in ALL {
            let mut words = finding(k).words();
            words.push(resolution_text(k, Resolution::NeedsConversation, ""));
            for w in words {
                let low = w.to_lowercase();
                for banned in ["risk", "score", "likelihood", "probab", "confidence", "%", "out of", "total", "suspicious", "misconduct", "cheat", "plagiar", "detected"] {
                    assert!(!low.contains(banned), "{k:?} says {banned:?}: {w}");
                }
                // The only digits are the recorded amounts the record itself quotes.
                if !w.starts_with("Event ") && !w.contains("characters") && !w.contains(METHOD_VERSION) {
                    assert!(!w.chars().any(|c| c.is_ascii_digit()), "{k:?} has a number: {w}");
                }
            }
        }
    }

    #[test]
    fn a_resolution_reads_with_its_note() {
        let t = resolution_text(FlagKind::LargePaste, Resolution::Explained, " The student showed the cited book. ");
        assert_eq!(t, "Finding · Large paste · findings.v1: Explained.\nThe student showed the cited book.");
        assert!(resolution_text(FlagKind::Paste, Resolution::Open, "").ends_with("Open."));
    }
}
