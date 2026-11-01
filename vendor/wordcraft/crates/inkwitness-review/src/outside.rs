//! Changes made outside InkWitness (FAL-664, GH #10): the file's text differs from the last state
//! its history replays to, so it was changed where the recorder wasn't running (Word, another
//! editor, a copy of the text). The history is still there to read; past its last event the
//! reviewer shows the document as the file has it, as a final section marked "Changed outside
//! InkWitness (not observed)", with what was added and removed there.
//!
//! Nothing is inferred about the time or the tool: the change is compared paragraph by paragraph
//! with the last recorded state, and large additions get the flags a large paste gets
//! ([`crate::flags`]), with the same wording rules and the same local checks (exact chatbot
//! phrases), and the sentence that timing and source are unknown. It is a flag with other
//! explanations, never a finding (`docs/adr/0005-changes-outside.md`).

use wordcraft_doc::StoryRef;
use wordcraft_inkwitness::PARA_MARK;
use wordcraft_inkwitness::replay::State;

use crate::card::count;
use crate::comments::{Anchor, MAX_QUOTE};
use crate::flags::{self, FlagKind};

/// The section's name, wherever it is shown.
pub const TITLE: &str = "Changed outside InkWitness (not observed)";

/// What it means, in the reviewer's words.
pub const MEANING: &str = "The file's text is not what the history ends with. InkWitness wasn't recording when it changed, so when it changed, and in which program, are unknown. Editing in Word after saving looks like this, and so does a copy of the text.";

/// The most paragraphs two sides are compared by line when looking for what stayed the same
/// (`old × new` cells of the comparison table); a longer change is shown as one block.
const MAX_CELLS: usize = 4_000_000;

/// One flag on a change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutsideFlag {
    /// A [`FlagKind`] with its words ([`FlagKind::meaning`]).
    pub kind: FlagKind,
    /// "Large addition outside InkWitness · 185 characters".
    pub label: String,
    /// The exact wording an [`FlagKind::AiPhrase`] matched.
    pub matched: Option<String>,
}

/// One contiguous change: what the recorded end had there, and what the file has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    /// The text the history ends with and the file doesn't (paragraph marks as [`PARA_MARK`]).
    pub removed: String,
    /// The text the file has and the history doesn't end with.
    pub added: String,
    /// Where it is in the file's document: the added text, or a point where text went.
    pub anchor: Option<Anchor>,
    pub flags: Vec<OutsideFlag>,
}

/// The file's document after the last recorded event, and what changed.
#[derive(Clone, Debug)]
pub struct Outside {
    /// The file's document, with block ids carried over from the recorded end where the
    /// paragraphs are the same, so a comment can be placed on it.
    pub state: State,
    pub hunks: Vec<Hunk>,
}

impl Outside {
    /// Compare the recorded end `end` with the file's `doc`. `None` when their text is the same.
    pub fn new(end: &State, doc: wordcraft_doc::Document) -> Option<Outside> {
        let state = end.with_outside_document(doc);
        let old: Vec<String> = end.paragraph_texts(StoryRef::Body).into_iter().map(|p| p.2).collect();
        let new = state.paragraph_texts(StoryRef::Body);
        let texts: Vec<&str> = new.iter().map(|p| p.2.as_str()).collect();
        let hunks = diff(&old, &texts)
            .into_iter()
            .filter_map(|h| {
                let anchor = (!new.is_empty()).then(|| {
                    let at = h.new_at.min(new.len() - 1);
                    // A change that begins in the middle of a paragraph starts that far in; a
                    // point (nothing added) sits at the start of the paragraph that follows.
                    let start = if h.added.is_empty() { 0 } else { h.skip };
                    Anchor { block_id: new[at].0.clone(), start, end: start + h.added.len(), quote: h.added.clone() }
                });
                let flags = flags_of(&h.added, &h.removed);
                (!(h.added.is_empty() && h.removed.is_empty())).then_some(Hunk { removed: h.removed, added: h.added, anchor, flags })
            })
            .collect::<Vec<_>>();
        (!hunks.is_empty()).then_some(Outside { state, hunks })
    }

    /// Characters added and removed over all the changes (paragraph marks not counted).
    pub fn totals(&self) -> (usize, usize) {
        let sum = |f: fn(&Hunk) -> &str| self.hunks.iter().map(|h| flags::measure(f(h)).0).sum();
        (sum(|h| &h.added), sum(|h| &h.removed))
    }

    /// One line for the status region and the rail.
    pub fn summary(&self) -> String {
        let (a, r) = self.totals();
        format!(
            "{TITLE}: {} added, {} removed. Timing and source are unknown.",
            count(a, "character", "characters"),
            count(r, "character", "characters")
        )
    }
}

/// A change before it is given its anchor and flags.
struct Raw {
    removed: String,
    added: String,
    /// The paragraph of the file the change starts in.
    new_at: usize,
    /// Bytes of that paragraph's flow the change starts after.
    skip: usize,
}

/// The paragraphs that changed between `old` and `new`, in order: the common start and end are
/// trimmed, what is left is compared by paragraph, and each run of paragraphs between two that
/// stayed the same is one change, narrowed to the characters that really differ.
fn diff(old: &[String], new: &[&str]) -> Vec<Raw> {
    let (m, n) = (old.len(), new.len());
    let mut pre = 0;
    while pre < m && pre < n && old[pre] == new[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < m - pre && suf < n - pre && old[m - 1 - suf] == new[n - 1 - suf] {
        suf += 1;
    }
    let (a, b) = (&old[pre..m - suf], &new[pre..n - suf]);
    let mut out = Vec::new();
    let mut run = |i0: usize, i1: usize, j0: usize, j1: usize| {
        if i0 == i1 && j0 == j1 {
            return;
        }
        out.push(narrow(&a[i0..i1], &b[j0..j1], pre + j0));
    };
    if (a.len() + 1).saturating_mul(b.len() + 1) > MAX_CELLS {
        run(0, a.len(), 0, b.len());
        return out;
    }
    // Longest common subsequence of paragraphs: `t[i][j]` is its length for a[i..] and b[j..].
    let w = b.len() + 1;
    let mut t = vec![0u32; (a.len() + 1) * w];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            t[i * w + j] = if a[i] == b[j] { t[(i + 1) * w + j + 1] + 1 } else { t[(i + 1) * w + j].max(t[i * w + j + 1]) };
        }
    }
    let (mut i, mut j, mut ci, mut cj) = (0, 0, 0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            run(ci, i, cj, j);
            i += 1;
            j += 1;
            (ci, cj) = (i, j);
        } else if t[(i + 1) * w + j] >= t[i * w + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    run(ci, a.len(), cj, b.len());
    out
}

/// One run of paragraphs `a` (before) and `b` (after, starting at paragraph `at` of the file),
/// narrowed to the characters between their common start and end.
fn narrow(a: &[String], b: &[&str], at: usize) -> Raw {
    let flow = |p: &mut dyn Iterator<Item = &str>| p.collect::<Vec<_>>().join(&PARA_MARK.to_string());
    let (fa, fb) = (flow(&mut a.iter().map(String::as_str)), flow(&mut b.iter().copied()));
    let mut p = fa.bytes().zip(fb.bytes()).take_while(|(x, y)| x == y).count();
    while p > 0 && !(fa.is_char_boundary(p) && fb.is_char_boundary(p)) {
        p -= 1;
    }
    let max = fa.len().min(fb.len()) - p;
    let mut s = fa.bytes().rev().zip(fb.bytes().rev()).take_while(|(x, y)| x == y).count().min(max);
    while s > 0 && !(fa.is_char_boundary(fa.len() - s) && fb.is_char_boundary(fb.len() - s)) {
        s -= 1;
    }
    let (removed, added) = (fa[p..fa.len() - s].to_string(), fb[p..fb.len() - s].to_string());
    // The change starts in the paragraph that holds its first character: skipping whole
    // paragraphs of common start moves the anchor on, so the offset stays inside one paragraph.
    let mut at = at;
    let mut skip = p;
    for para in b {
        if skip > para.len() && b.len() > 1 {
            skip -= para.len() + PARA_MARK.len_utf8();
            at += 1;
        } else {
            break;
        }
    }
    Raw { removed, added, new_at: at, skip }
}

/// The flags of a change that added `added` and removed `removed`, strongest first.
pub fn flags_of(added: &str, removed: &str) -> Vec<OutsideFlag> {
    let (ic, iw) = flags::measure(added);
    let (dc, _) = flags::measure(removed);
    let mut out = Vec::new();
    if ic > 0
        && let Some(m) = flags::template_marker(added)
    {
        out.push(OutsideFlag { kind: FlagKind::AiPhrase, label: format!("AI-style phrase · “{m}”"), matched: Some(m) });
    }
    if ic >= flags::LARGE_PASTE_CHARS || iw >= flags::LARGE_PASTE_WORDS {
        out.push(OutsideFlag {
            kind: FlagKind::OutsideAddition,
            label: format!("Large addition outside InkWitness · {}", count(ic, "character", "characters")),
            matched: None,
        });
    }
    if dc >= flags::LARGE_DELETE_CHARS {
        out.push(OutsideFlag {
            kind: FlagKind::LargeDelete,
            label: format!("Large deletion · {}", count(dc, "character", "characters")),
            matched: None,
        });
    }
    out
}

/// The template a promoted draft starts from, for flag `kind` (`None`: any other change). Held
/// to the same wording rules as [`crate::comments::template`], and every one says that the time
/// isn't known.
pub fn template(kind: Option<FlagKind>) -> &'static str {
    match kind {
        Some(FlagKind::OutsideAddition) => {
            "This passage was added to the file where InkWitness wasn't recording: {size}.\n{quote}\nWhen it was added and where it came from isn't known. Where did this passage come from, and how did it get into the file?"
        }
        Some(FlagKind::AiPhrase) => {
            "This passage, added to the file where InkWitness wasn't recording, includes the wording “{matched}”.\n{quote}\nWhen it was added isn't known. Where does this wording come from? Can you tell me how you wrote this passage?"
        }
        Some(FlagKind::LargeDelete) => {
            "A long passage that is in the recorded history is not in the file: {size}.\n{quote}\nWhen it was removed isn't known. What made you decide to take this part out?"
        }
        _ => "The file was changed where InkWitness wasn't recording: {size}.\n{quote}\nWhen and how isn't known. Can you tell me about this change?",
    }
}

/// Every template, for the wording test.
pub const ALL_KINDS: [Option<FlagKind>; 4] = [Some(FlagKind::OutsideAddition), Some(FlagKind::AiPhrase), Some(FlagKind::LargeDelete), None];

/// A promoted draft's text for `hunk`, from its flag `flag` (`None`: the change as a whole).
pub fn promote_text(hunk: &Hunk, flag: Option<&OutsideFlag>) -> String {
    let kind = flag.map(|f| f.kind);
    let text = if kind == Some(FlagKind::LargeDelete) || hunk.added.is_empty() { &hunk.removed } else { &hunk.added };
    let flat = text.replace(PARA_MARK, "\n");
    let flat = crate::card::unquote(flat.trim());
    let quote = if flat.is_empty() {
        String::new()
    } else if flat.chars().count() > MAX_QUOTE {
        format!("“{}…”", flat.chars().take(MAX_QUOTE).collect::<String>().trim_end())
    } else {
        format!("“{flat}”")
    };
    let size = crate::rail::size(&hunk.added, &hunk.removed).unwrap_or_else(|| "no text changed".into());
    template(kind)
        .replace("{size}", &size)
        .replace("{matched}", flag.and_then(|f| f.matched.as_deref()).unwrap_or_default())
        .replace("{quote}", &quote)
        .replace("\n\n", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(old: &[&str], new: &[&str]) -> Vec<(String, String)> {
        let old: Vec<String> = old.iter().map(|s| s.to_string()).collect();
        diff(&old, new).into_iter().map(|h| (h.removed, h.added)).collect()
    }

    #[test]
    fn nothing_changed_is_no_change() {
        assert!(d(&["a", "b"], &["a", "b"]).is_empty());
        assert!(d(&[], &[]).is_empty());
    }

    #[test]
    fn an_added_paragraph_and_a_changed_word_are_found_where_they_are() {
        assert_eq!(d(&["one", "two"], &["one", "two", "three"]), vec![(String::new(), "three".into())]);
        assert_eq!(d(&["the quick fox", "end"], &["the slow fox", "end"]), vec![("quick".into(), "slow".into())]);
        // Two separate edits are two changes, not one big one.
        let got = d(&["a1", "same", "b1"], &["a2", "same", "b2"]);
        assert_eq!(got, vec![("1".into(), "2".into()), ("1".into(), "2".into())]);
    }

    #[test]
    fn a_removed_block_has_nothing_added() {
        let got = d(&["keep", "gone one", "gone two", "keep too"], &["keep", "keep too"]);
        assert_eq!(got.len(), 1);
        assert!(got[0].1.is_empty());
        assert!(got[0].0.contains("gone one") && got[0].0.contains("gone two"));
    }

    #[test]
    fn a_big_addition_gets_the_paste_flag_and_chatbot_wording_is_quoted() {
        let long = "word ".repeat(40);
        let f = flags_of(&long, "");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, FlagKind::OutsideAddition);
        let f = flags_of("Certainly! Here is the paragraph.", "");
        assert_eq!(f[0].kind, FlagKind::AiPhrase);
        assert_eq!(f[0].matched.as_deref(), Some("Certainly! Here"));
        // A small edit is not flagged.
        assert!(flags_of("slow", "quick").is_empty());
        assert_eq!(flags_of("", &"x".repeat(flags::LARGE_DELETE_CHARS))[0].kind, FlagKind::LargeDelete);
    }

    #[test]
    fn templates_never_accuse_and_always_say_the_time_is_unknown() {
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
                "misconduct",
                "plagiarism",
                "probability",
                "suspicious",
                "detected",
                "digest",
                "hash",
            ] {
                assert!(!words.iter().any(|w| w == banned), "{k:?} says {banned:?}: {t}");
            }
            assert!(t.trim_end().ends_with('?'), "{k:?} ends with a neutral question: {t}");
            assert!(t.contains("isn't known"), "{k:?} says the time isn't known: {t}");
        }
        let low = format!("{TITLE} {MEANING}").to_lowercase();
        for banned in ["suspicious", "misconduct", "cheat", "probability", "detected", "plagiar"] {
            assert!(!low.contains(banned));
        }
    }
}
