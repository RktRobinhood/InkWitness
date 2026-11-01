//! Teacher comments (FAL-519, from FAL-449 and FAL-450) and highlights (FAL-547): a review layer
//! kept apart from the student's document and its evidence. A comment is anchored in the **final** recorded document
//! by block id and the commented text, so it can be found again in any replayed state where that
//! text exists, and it can point back to the replay moment it was made at or promoted from.
//!
//! Promote to comment turns a flag or an event into an editable draft, pre-filled from the
//! evidence with what happened, when, how much, the passage and one neutral question. The
//! wording rules of `docs/REPLAY_UI.md` hold for every template ([`template`]): no AI, no
//! misconduct, no internal values. Nothing becomes a comment until the teacher saves the draft.

use serde_json::Value;
use wordcraft_doc::{Pos, StoryRef};
use wordcraft_inkwitness::PARA_MARK;
use wordcraft_inkwitness::replay::State;

use crate::flags::FlagKind;
use crate::review::Loaded;

/// Where a comment sits: a range of text from the start of one paragraph's block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anchor {
    /// The block the range starts in.
    pub block_id: String,
    /// Byte offsets from the start of that block, paragraph marks counting as [`PARA_MARK`].
    pub start: usize,
    pub end: usize,
    /// The commented text (paragraph marks as [`PARA_MARK`]); empty for a point.
    pub quote: String,
}

/// A teacher's comment, or highlight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeacherComment {
    pub id: u32,
    /// The comment's text (empty for a highlight).
    pub text: String,
    /// A highlight in this colour, not a comment: painted over its text on the page, and an
    /// ordinary Word highlight in the review copy.
    pub highlight: Option<wordcraft_doc::props::Highlight>,
    /// Where it sits in the final document (`None`: the document has no recorded blocks).
    pub anchor: Option<Anchor>,
    /// The commented text no longer exists in the final document: the comment sits at the
    /// nearest surviving place and quotes it.
    pub moved: bool,
    /// The replay moment it is about.
    pub seq: Option<u64>,
    /// The flag it was promoted from, if any.
    pub from_flag: Option<(u64, FlagKind)>,
    /// When it was written (RFC 3339), as the comment's date in the review copy.
    pub written: String,
    /// The teacher's resolution of the finding at `from_flag` (FAL-665): this is then the
    /// resolution and its note, not a comment of its own.
    pub resolution: Option<crate::findings::Resolution>,
}

impl TeacherComment {
    /// The text the comment has on the page and in the review copy.
    pub fn shown_text(&self) -> String {
        match (self.resolution, self.from_flag) {
            (Some(r), Some((_, k))) => crate::findings::resolution_text(k, r, &self.text),
            _ => self.text.clone(),
        }
    }
}

/// A comment being written or edited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    /// The comment it edits (`None`: a new one).
    pub editing: Option<u32>,
    pub text: String,
    pub anchor: Option<Anchor>,
    pub moved: bool,
    pub seq: Option<u64>,
    pub from_flag: Option<(u64, FlagKind)>,
    /// Give the text box the keyboard on the next frame.
    pub focus: bool,
}

impl Draft {
    pub fn of(c: &TeacherComment) -> Draft {
        Draft { editing: Some(c.id), text: c.text.clone(), anchor: c.anchor.clone(), moved: c.moved, seq: c.seq, from_flag: c.from_flag, focus: true }
    }
}

/// The flow of story `s`'s paragraphs from block `id` on (paragraph marks between them), and
/// each paragraph's block id and byte offset in it.
fn flow_from(st: &State, s: StoryRef, id: &str, max_paras: usize) -> Option<(String, Vec<(String, usize)>)> {
    let paras = st.paragraph_texts(s);
    let k = paras.iter().position(|(b, _, _)| b == id)?;
    let mut flow = String::new();
    let mut at = Vec::new();
    for (i, (b, _, t)) in paras.iter().skip(k).take(max_paras).enumerate() {
        if i > 0 {
            flow.push(PARA_MARK);
        }
        at.push((b.clone(), flow.len()));
        flow.push_str(t);
    }
    Some((flow, at))
}

/// The most paragraphs a comment's text is looked for across.
const SEARCH_PARAS: usize = 40;

/// Where `anchor`'s text is in `st`, as byte offsets in the flow from its block: its quote,
/// nearest its recorded offset. A point (empty quote) sits at its offset, kept inside the block.
/// `None` when the block or the text isn't there: a comment is never attached to an unrelated
/// match elsewhere.
pub fn find(st: &State, anchor: &Anchor) -> Option<(usize, usize)> {
    let (flow, _) = flow_from(st, StoryRef::Body, &anchor.block_id, SEARCH_PARAS)?;
    if anchor.quote.is_empty() {
        let first = flow.split(PARA_MARK).next().unwrap_or_default();
        let mut o = anchor.start.min(first.len());
        while !first.is_char_boundary(o) {
            o -= 1;
        }
        return Some((o, o));
    }
    let s = flow.match_indices(anchor.quote.as_str()).map(|(i, _)| i).min_by_key(|i| i.abs_diff(anchor.start))?;
    Some((s, s + anchor.quote.len()))
}

/// Where `anchor` is in `st`, as positions ([`find`]).
pub fn locate(st: &State, anchor: &Anchor) -> Option<(Pos, Pos)> {
    let (s, e) = find(st, anchor)?;
    let r = st.resolve(&serde_json::json!({"blockId": anchor.block_id, "startOffset": s, "endOffset": e}), None)?;
    Some((r.start, r.end))
}

/// The shortest part of a passage (in characters) a comment is carried forward on alone.
pub const MIN_PIECE: usize = 24;

/// The anchor for `start..end` as shown in `from` (an earlier state, or the final one), carried
/// forward to `last`, the final recorded state: on the same text, or else on the longest of its
/// paragraphs that survives. When none of the text exists there, the anchor is a point at the
/// start of the nearest paragraph that survives, and `moved` is true.
pub fn anchor_forward(from: &State, last: &State, start: &Pos, end: &Pos) -> Option<(Anchor, bool)> {
    let quote = from.text(start, end);
    let id = from.block_at(start)?;
    let here = Anchor { block_id: id.clone(), start: start.off, end: start.off + quote.len(), quote: quote.clone() };
    if let Some((s, e)) = find(last, &here) {
        return Some((Anchor { block_id: id, start: s, end: e, quote }, false));
    }
    // Part of it survives (a pasted answer whose first and last lines were cut): the longest of
    // its paragraphs found whole in the final document. Only a paragraph long enough not to
    // match unrelated text elsewhere.
    let mut pieces: Vec<&str> = quote.split(PARA_MARK).filter(|p| p.chars().count() >= MIN_PIECE).collect();
    pieces.sort_by_key(|p| std::cmp::Reverse(p.len()));
    let finals = last.paragraph_texts(StoryRef::Body);
    for piece in pieces {
        if let Some((b, i)) = finals.iter().find_map(|(b, _, t)| t.find(piece).map(|i| (b.clone(), i))) {
            return Some((Anchor { block_id: b, start: i, end: i + piece.len(), quote: piece.to_string() }, false));
        }
    }
    // The text is gone: the nearest surviving paragraph, looking forward first.
    let before = from.paragraph_texts(StoryRef::Body);
    let after: Vec<String> = last.paragraph_texts(StoryRef::Body).into_iter().map(|(b, _, _)| b).collect();
    let k = before.iter().position(|(b, _, _)| *b == id)?;
    let survivor = before.iter().skip(k).chain(before.iter().take(k).rev()).map(|(b, _, _)| b).find(|b| after.contains(b)).or(after.first())?;
    Some((Anchor { block_id: survivor.clone(), start: 0, end: 0, quote: String::new() }, true))
}

/// The longest passage a promoted draft quotes, in characters (the teacher can trim it).
pub const MAX_QUOTE: usize = 240;

/// The template a promoted draft starts from, for a flag of `kind` (`None`: any other event).
/// Placeholders: `{when}`, `{size}`, `{quote}`, `{matched}`, `{what}`. Every template is held to
/// the wording rules by a test: no AI, no misconduct, no internal values.
pub fn template(kind: Option<FlagKind>) -> &'static str {
    match kind {
        Some(FlagKind::Paste) => "Pasted in on {when}: {size}.\n{quote}\nWhere did this passage come from, and why did you decide to use it here?",
        Some(FlagKind::LargePaste) => {
            "A long passage was pasted in on {when}: {size}.\n{quote}\nWhere did this passage come from? Can you explain it in your own words?"
        }
        Some(FlagKind::LargeInsert) => {
            "This passage arrived in one step on {when}, rather than being typed here: {size}.\n{quote}\nCan you walk me through how you wrote this part?"
        }
        Some(FlagKind::LargeDelete) => "A long passage was removed on {when}: {size}.\n{quote}\nWhat made you decide to take this part out?",
        Some(FlagKind::Imported) => {
            "This text was already in the document when it was opened: {size}.\n{quote}\nWhere and when did you write this part?"
        }
        Some(FlagKind::AiPhrase) => {
            "This passage, added on {when}, includes the wording “{matched}”.\n{quote}\nWhere does this wording come from? Can you tell me how you wrote this passage?"
        }
        Some(FlagKind::OutsideAddition) => crate::outside::template(Some(FlagKind::OutsideAddition)),
        Some(FlagKind::HistoryStops) => {
            "The recorded writing history stops here, on {when}.\nCan you tell me what happened to the file after this point?"
        }
        None => "On {when}: {what}, {size}.\n{quote}\nCan you tell me about this change?",
    }
}

/// Every template, for the wording test.
pub const ALL_KINDS: [Option<FlagKind>; 9] = [
    Some(FlagKind::Paste),
    Some(FlagKind::LargePaste),
    Some(FlagKind::LargeInsert),
    Some(FlagKind::LargeDelete),
    Some(FlagKind::Imported),
    Some(FlagKind::AiPhrase),
    Some(FlagKind::HistoryStops),
    Some(FlagKind::OutsideAddition),
    None,
];

/// A promoted draft's text for event `seq`, from the flag `kind` it was promoted from.
pub fn promote_text(l: &Loaded, seq: u64, kind: Option<FlagKind>, matched: Option<&str>) -> String {
    let Some(e) = l.replay.event(seq) else { return String::new() };
    let s = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or_default();
    let mut when = s("atLocalIso").is_empty().then(String::new).unwrap_or_else(|| crate::rail::recorded(s("atLocalIso")));
    if let Some(ms) = e.get("elapsedMonotonicMs").and_then(Value::as_u64) {
        let el = crate::rail::elapsed(ms);
        when = if when.is_empty() { el } else { format!("{when} ({el})") };
    }
    let (ins, del) = (s("insertedText"), s("deletedText"));
    let size = crate::rail::size(ins, del).unwrap_or_else(|| "no text changed".into());
    let text = if ins.is_empty() { del } else { ins };
    let flat = text.replace(PARA_MARK, "\n");
    let flat = crate::card::unquote(flat.trim());
    let quote = if flat.is_empty() {
        String::new()
    } else if flat.chars().count() > MAX_QUOTE {
        format!("“{}…”", flat.chars().take(MAX_QUOTE).collect::<String>().trim_end())
    } else {
        format!("“{flat}”")
    };
    let what = crate::rail::kind_of(s("type")).1.to_lowercase();
    let out = template(kind)
        .replace("{when}", &when)
        .replace("{size}", &size)
        .replace("{matched}", matched.unwrap_or_default())
        .replace("{what}", &what)
        .replace("{quote}", &quote);
    // An empty quote leaves an empty line.
    out.replace("\n\n", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_never_mention_ai_misconduct_or_internal_values() {
        for k in ALL_KINDS {
            let t = template(k);
            let words: Vec<String> = t.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect();
            for banned in [
                "ai",
                "chatgpt",
                "chatbot",
                "gpt",
                "artificial",
                "cheat",
                "cheating",
                "cheated",
                "misconduct",
                "plagiarism",
                "plagiarised",
                "plagiarized",
                "probability",
                "suspicious",
                "detected",
                "gestureid",
                "hash",
                "seq",
                "digest",
            ] {
                assert!(!words.iter().any(|w| w == banned), "{k:?} says {banned:?}: {t}");
            }
            assert!(t.trim_end().ends_with('?'), "{k:?} ends with a neutral question: {t}");
        }
    }
}
