//! Block identity and the structural diff behind every event (`docs/adr/0001-event-anchors.md`
//! in the InkWitness repository).
//!
//! WordCraft's paragraphs carry no stable id, so the recorder keeps one per paragraph beside the
//! document, in document order per story. Each commit diffs the paragraphs before and after:
//! unchanged ones keep their ids, and the changed window hands ids on by where the text went
//! (a split keeps the id with the part holding the paragraph's first surviving character, a
//! merge keeps the first paragraph's id). Offsets are UTF-8 bytes in the paragraph's model text
//! (`Paragraph::text`, as `Pos::off`), never pixels.

use std::collections::BTreeMap;
use std::sync::Arc;

use wordcraft_doc::{Block, Blocks, Document, Pos, StoryRef};

/// The paragraph mark between paragraphs in a window's flow text. Paragraph text never holds it
/// (line breaks are `\n`), so a split or merge shows up as exactly this one character.
pub const PARA_MARK: char = '\u{2029}';

/// Context kept on each side of an anchor for its context hash, in characters.
const CONTEXT_CHARS: usize = 16;

/// Tables inside tables are walked this deep at most.
const MAX_NESTING: usize = 32;

/// A story's name in events: `body` or `part:<id>`.
pub fn story_key(s: StoryRef) -> String {
    match s {
        StoryRef::Body => "body".into(),
        StoryRef::Part(n) => format!("part:{n}"),
    }
}

/// The stories of a document in a fixed order: the body, then the parts by id.
pub(crate) fn stories(doc: &Document) -> Vec<(StoryRef, &Blocks)> {
    let mut out = vec![(StoryRef::Body, &doc.body)];
    out.extend(doc.parts.iter().map(|(k, p)| (StoryRef::Part(*k), &p.blocks)));
    out
}

/// A paragraph and its path in its story.
pub(crate) type Para<'a> = (Vec<u32>, &'a Arc<Block>);

/// Every paragraph of a story in document order, table cells included.
pub(crate) fn paragraphs(blocks: &Blocks) -> Vec<Para<'_>> {
    let mut out = Vec::new();
    walk(blocks, &mut Vec::new(), &mut out);
    out
}

fn walk<'a>(blocks: &'a Blocks, path: &mut Vec<u32>, out: &mut Vec<Para<'a>>) {
    for (i, b) in blocks.iter().enumerate() {
        path.push(u32::try_from(i).unwrap_or(u32::MAX));
        match &**b {
            Block::Para(_) => out.push((path.clone(), b)),
            Block::Table(t) if path.len() / 3 < MAX_NESTING => {
                for (r, row) in t.rows.iter().enumerate() {
                    for (c, cell) in row.cells.iter().enumerate() {
                        path.push(u32::try_from(r).unwrap_or(u32::MAX));
                        path.push(u32::try_from(c).unwrap_or(u32::MAX));
                        walk(&cell.blocks, path, out);
                        path.pop();
                        path.pop();
                    }
                }
            }
            Block::Table(_) => {}
        }
        path.pop();
    }
}

fn text(b: &Block) -> &str {
    b.as_para().map_or("", |p| p.text.as_str())
}

fn same(a: &Para<'_>, b: &Para<'_>) -> bool {
    Arc::ptr_eq(a.1, b.1) || **a.1 == **b.1
}

/// Hands out block ids: `b1`, `b2`… in the order blocks first appear.
#[derive(Debug, Default)]
pub(crate) struct Ids {
    next: u64,
    /// Each story's paragraph ids, in document order (aligned with [`paragraphs`]).
    pub stories: BTreeMap<StoryRef, Vec<String>>,
}

impl Ids {
    fn fresh(&mut self) -> String {
        self.next = self.next.saturating_add(1);
        format!("b{}", self.next)
    }

    /// Each story's ids by story name (`body`, `part:<id>`).
    pub fn keyed(&self) -> BTreeMap<String, Vec<String>> {
        self.stories.iter().map(|(s, ids)| (story_key(*s), ids.clone())).collect()
    }

    /// Fresh ids for every paragraph of `doc` (the first document, or one that replaced it).
    pub fn assign_all(&mut self, doc: &Document) {
        let mut all = BTreeMap::new();
        for (s, blocks) in stories(doc) {
            let ids = (0..paragraphs(blocks).len()).map(|_| self.fresh()).collect();
            all.insert(s, ids);
        }
        self.stories = all;
    }
}

/// A range in model units: a block id, and UTF-8 byte offsets from that block's start. A range
/// that runs on past the block's end continues through the following paragraphs of the story,
/// each paragraph mark counting as [`PARA_MARK`]'s three bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub block_id: String,
    pub start: usize,
    pub end: usize,
    /// SHA-256 of the text just around the range (see [`crate::context_hash`]).
    pub context_hash: String,
}

/// One block of a window: its id and text length (bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowBlock {
    pub id: String,
    pub len: usize,
}

/// One contiguous change in one story: the window of paragraphs it replaced, and the text
/// change within the window's flow (paragraph texts joined by [`PARA_MARK`]). Reversible: the
/// window before is the window after with `inserted` at `offset` put back to `deleted`.
#[derive(Clone, Debug)]
pub struct Op {
    pub story: StoryRef,
    pub before: Vec<WindowBlock>,
    pub after: Vec<WindowBlock>,
    /// Byte offset in the window's flow, the same before and after.
    pub offset: usize,
    pub deleted: String,
    pub inserted: String,
    pub before_anchor: Option<Anchor>,
    pub after_anchor: Option<Anchor>,
    /// The window's paragraphs before the change.
    pub before_blocks: Vec<Arc<Block>>,
    /// The window's paragraphs after the change, for an exact replay of formatting.
    pub after_blocks: Vec<Arc<Block>>,
    /// Whether the window's paragraphs are one run of neighbouring blocks in one container, before
    /// and after. When either is not (a table came or went in the window), replacing the window's
    /// paragraphs can't rebuild the story, so the event carries the whole story instead.
    pub siblings: bool,
}

/// Where the change most likely is, from the selection: the start of the selection before the
/// command, and the caret after it. Used only to choose between equally small diffs ("one two
/// three" losing "two " could also be "wo t" losing at another place).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Hints<'a> {
    pub before: Option<&'a Pos>,
    pub after: Option<&'a Pos>,
}

/// Diff `old` (whose ids are `ids`) against `new`, hand the ids on, and return the changes.
pub(crate) fn rebind(ids: &mut Ids, old: &Document, new: &Document, hints: Hints<'_>) -> Vec<Op> {
    let mut ops = Vec::new();
    let old_stories: BTreeMap<StoryRef, &Blocks> = stories(old).into_iter().collect();
    let mut next = BTreeMap::new();
    for (s, blocks) in stories(new) {
        let after = paragraphs(blocks);
        let before = old_stories.get(&s).map(|b| paragraphs(b)).unwrap_or_default();
        let before_ids = ids.stories.get(&s).cloned().unwrap_or_default();
        let hints = Hints { before: hints.before.filter(|p| p.story == s), after: hints.after.filter(|p| p.story == s) };
        let after_ids = story_diff(ids, s, &before, &before_ids, &after, hints, &mut ops);
        next.insert(s, after_ids);
    }
    // A story that went away: its paragraphs were all deleted.
    for (s, blocks) in &old_stories {
        if !next.contains_key(s) {
            let before = paragraphs(blocks);
            let before_ids = ids.stories.get(s).cloned().unwrap_or_default();
            story_diff(ids, *s, &before, &before_ids, &[], Hints::default(), &mut ops);
        }
    }
    ids.stories = next;
    ops
}

/// The id of paragraph `i` before, or a fresh one if the recorder lost track (it never should).
fn id_at(ids: &mut Ids, before_ids: &[String], i: usize) -> String {
    match before_ids.get(i) {
        Some(id) => id.clone(),
        None => ids.fresh(),
    }
}

fn story_diff(
    ids: &mut Ids,
    story: StoryRef,
    before: &[Para<'_>],
    before_ids: &[String],
    after: &[Para<'_>],
    hints: Hints<'_>,
    ops: &mut Vec<Op>,
) -> Vec<String> {
    let (m, n) = (before.len(), after.len());
    let mut pre = 0;
    while pre < m && pre < n && same(&before[pre], &after[pre]) {
        pre += 1;
    }
    let mut suf = 0;
    while suf < m - pre && suf < n - pre && same(&before[m - 1 - suf], &after[n - 1 - suf]) {
        suf += 1;
    }
    let mut out: Vec<String> = (0..pre).map(|i| id_at(ids, before_ids, i)).collect();
    if pre == m && pre == n {
        return out;
    }
    let (mut lo, mut hi_b, mut hi_a) = (pre, m - suf, n - suf);
    if hi_b - lo == hi_a - lo {
        // As many paragraphs as before: each changed one is its own change and keeps its id.
        for i in lo..hi_b {
            let id = id_at(ids, before_ids, i);
            if !same(&before[i], &after[i]) {
                ops.push(window_op(story, &before[i..=i], std::slice::from_ref(&id), &after[i..=i], std::slice::from_ref(&id), hints));
            }
            out.push(id);
        }
    } else {
        // Paragraphs came or went. A window empty on one side takes in a neighbour, so the
        // paragraph mark that came or went has text on both sides to sit between.
        if lo == hi_b || lo == hi_a {
            if lo > 0 {
                lo -= 1;
                out.pop();
            } else if hi_b < m && hi_a < n {
                hi_b += 1;
                hi_a += 1;
                suf -= 1;
            }
        }
        let old_ids: Vec<String> = (lo..hi_b).map(|i| id_at(ids, before_ids, i)).collect();
        let new_ids = inherit(ids, &before[lo..hi_b], &old_ids, &after[lo..hi_a], hints);
        ops.push(window_op(story, &before[lo..hi_b], &old_ids, &after[lo..hi_a], &new_ids, hints));
        out.extend(new_ids);
    }
    out.extend((m - suf..m).map(|i| id_at(ids, before_ids, i)));
    out
}

/// A window's flow text and where each paragraph starts in it.
fn flow(blocks: &[Para<'_>]) -> (String, Vec<usize>) {
    let mut s = String::new();
    let mut starts = Vec::with_capacity(blocks.len());
    for (i, (_, b)) in blocks.iter().enumerate() {
        if i > 0 {
            s.push(PARA_MARK);
        }
        starts.push(s.len());
        s.push_str(text(b));
    }
    (s, starts)
}

/// The change from `a` to `b` as (start, end in `a`, end in `b`): the common prefix and suffix,
/// on character boundaries.
pub(crate) fn text_diff(a: &str, b: &str) -> (usize, usize, usize) {
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    let min = ab.len().min(bb.len());
    let mut p = 0;
    while p < min && ab.get(p) == bb.get(p) {
        p += 1;
    }
    while p > 0 && !(a.is_char_boundary(p) && b.is_char_boundary(p)) {
        p -= 1;
    }
    let mut s = 0;
    while s < min - p && ab.get(ab.len() - 1 - s) == bb.get(bb.len() - 1 - s) {
        s += 1;
    }
    while s > 0 && !(a.is_char_boundary(ab.len() - s) && b.is_char_boundary(bb.len() - s)) {
        s -= 1;
    }
    (p, ab.len() - s, bb.len() - s)
}

/// A hint's place in a window's flow, if it is in one of the window's paragraphs.
fn hint_offset(window: &[Para<'_>], starts: &[usize], hint: Option<&Pos>) -> Option<usize> {
    let h = hint?;
    let k = window.iter().position(|(p, _)| *p == h.path.0)?;
    starts.get(k).map(|s| s + h.off)
}

/// The change from window `before` to window `after` as (start, end before, end after). Of the
/// equally small diffs (the change slides left while the text it leaves matches), the one at the
/// selection before the command is taken, else the one ending at the caret after it, else the
/// rightmost.
fn change(before: &[Para<'_>], after: &[Para<'_>], hints: Hints<'_>) -> (String, Vec<usize>, String, Vec<usize>, (usize, usize, usize)) {
    let (a, a_starts) = flow(before);
    let (b, b_starts) = flow(after);
    let (start, del_end, ins_end) = text_diff(&a, &b);
    let (del, ins) = (del_end - start, ins_end - start);
    // Every start the change can slide to, rightmost first.
    let mut starts = vec![start];
    let mut k = start;
    while let Some(c) = a.get(..k).and_then(|h| h.chars().next_back()) {
        let l = c.len_utf8();
        let fits = |s: &str, end: usize, len: usize| len == 0 || (end >= l && s.get(end - l..end).is_some_and(|x| x.starts_with(c)));
        if !(fits(&a, k + del, del) && fits(&b, k + ins, ins)) {
            break;
        }
        k -= l;
        starts.push(k);
    }
    let at_before = hint_offset(before, &a_starts, hints.before);
    let at_after = hint_offset(after, &b_starts, hints.after).and_then(|x| x.checked_sub(ins));
    let s = [at_before, at_after].into_iter().flatten().find(|h| starts.contains(h)).unwrap_or(start);
    (a, a_starts, b, b_starts, (s, s + del, s + ins))
}

/// Ids for the window after: each paragraph before hands its id to the paragraph after that
/// holds its first surviving character (an empty paragraph: its place); paragraphs left over
/// on both sides pair up in order; the rest get fresh ids.
fn inherit(ids: &mut Ids, before: &[Para<'_>], before_ids: &[String], after: &[Para<'_>], hints: Hints<'_>) -> Vec<String> {
    let (_, a_starts, _, b_starts, (start, del_end, ins_end)) = change(before, after, hints);
    let map = |x: usize| {
        if x < start {
            Some(x)
        } else if x >= del_end {
            Some(x - del_end + ins_end)
        } else {
            None
        }
    };
    let mut claimed: Vec<Option<String>> = vec![None; after.len()];
    let mut unclaimed_old = Vec::new();
    // Paragraphs with text first: their text says where they went. An empty one only has a
    // place, which a paragraph mark next to it makes ambiguous.
    let mut order: Vec<usize> = (0..before.len()).filter(|&k| before.get(k).is_some_and(|p| !text(p.1).is_empty())).collect();
    order.extend((0..before.len()).filter(|&k| before.get(k).is_some_and(|p| text(p.1).is_empty())));
    for k in order {
        let Some((_, blk)) = before.get(k) else { continue };
        let s = a_starts.get(k).copied().unwrap_or(0);
        let e = s + text(blk).len();
        let target = if s == e {
            // An empty paragraph stays on the near side of an insertion at its place.
            let y = if s <= start { Some(s) } else { map(s) };
            y.and_then(|y| {
                (0..after.len()).find(|&j| b_starts.get(j).is_some_and(|&bs| bs <= y && y <= bs + after.get(j).map_or(0, |p| text(p.1).len())))
            })
        } else {
            let x = if s < start || s >= del_end { Some(s) } else { (del_end < e).then_some(del_end) };
            x.and_then(map).and_then(|y| {
                (0..after.len()).find(|&j| b_starts.get(j).is_some_and(|&bs| bs <= y && y < bs + after.get(j).map_or(0, |p| text(p.1).len())))
            })
        };
        match target {
            Some(j) if claimed.get(j).is_some_and(Option::is_none) => {
                if let Some(c) = claimed.get_mut(j) {
                    *c = before_ids.get(k).cloned();
                }
            }
            _ => unclaimed_old.push(k),
        }
    }
    let mut left = unclaimed_old.into_iter();
    claimed
        .into_iter()
        .map(|c| match c {
            Some(id) => id,
            None => match left.next().and_then(|k| before_ids.get(k).cloned()) {
                Some(id) => id,
                None => ids.fresh(),
            },
        })
        .collect()
}

fn window_op(story: StoryRef, before: &[Para<'_>], before_ids: &[String], after: &[Para<'_>], after_ids: &[String], hints: Hints<'_>) -> Op {
    let (a, a_starts, b, b_starts, (start, del_end, ins_end)) = change(before, after, hints);
    let deleted = a.get(start..del_end).unwrap_or_default().to_string();
    let inserted = b.get(start..ins_end).unwrap_or_default().to_string();
    let lens = |bl: &[Para<'_>], ids: &[String]| -> Vec<WindowBlock> {
        bl.iter().zip(ids).map(|((_, b), id)| WindowBlock { id: id.clone(), len: text(b).len() }).collect()
    };
    Op {
        story,
        before_anchor: anchor(&a, &a_starts, before_ids, start, del_end),
        after_anchor: anchor(&b, &b_starts, after_ids, start, ins_end),
        before: lens(before, before_ids),
        after: lens(after, after_ids),
        offset: start,
        deleted,
        inserted,
        before_blocks: before.iter().map(|(_, b)| Arc::clone(b)).collect(),
        after_blocks: after.iter().map(|(_, b)| Arc::clone(b)).collect(),
        siblings: sibling_run(before) && sibling_run(after),
    }
}

/// Whether `window` is one run of neighbouring blocks in one container (same parent path,
/// consecutive indexes). An empty window is.
pub(crate) fn sibling_run(window: &[Para<'_>]) -> bool {
    window.windows(2).all(|w| match (w[0].0.split_last(), w[1].0.split_last()) {
        (Some((a, pa)), Some((b, pb))) => pa == pb && a.checked_add(1) == Some(*b),
        _ => false,
    })
}

/// The range `start..end` of a window's flow, anchored on the paragraph it starts in.
fn anchor(flow: &str, starts: &[usize], ids: &[String], start: usize, end: usize) -> Option<Anchor> {
    // The last paragraph starting at or before `start`: a range starting right after a
    // paragraph mark belongs to the next paragraph.
    let k = starts.iter().rposition(|&s| s <= start)?;
    let base = starts.get(k).copied()?;
    let before: String = {
        let head = flow.get(..start).unwrap_or_default();
        let mut c: Vec<char> = head.chars().rev().take(CONTEXT_CHARS).collect();
        c.reverse();
        c.into_iter().collect()
    };
    let after: String = flow.get(end..).unwrap_or_default().chars().take(CONTEXT_CHARS).collect();
    Some(Anchor { block_id: ids.get(k)?.clone(), start: start - base, end: end - base, context_hash: crate::context_hash(&before, &after) })
}

#[cfg(test)]
mod tests {
    use super::text_diff;

    #[test]
    fn text_diff_respects_char_boundaries() {
        assert_eq!(text_diff("abc", "abXc"), (2, 2, 3));
        assert_eq!(text_diff("", "abc"), (0, 0, 3));
        assert_eq!(text_diff("aaa", "aa"), (2, 3, 2));
        // é (C3 A9) and è (C3 A8) share a first byte: the change is the whole character.
        assert_eq!(text_diff("é", "è"), (0, 2, 2));
        assert_eq!(text_diff("aé", "a"), (1, 3, 1));
    }
}
