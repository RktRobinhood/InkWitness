//! Teacher comments as WordCraft comments (FAL-519): drawn on the page as comment balloons, and
//! written into a separate review copy as Word comments. The student's file is never written.
//!
//! The comments are added to a copy of the shown document only ([`decorate`]). Each one puts two
//! objects into the text (its start and end), so offsets on the page and in the replayed state
//! differ by them; [`to_shown`] and [`from_shown`] convert between the two.

use std::collections::BTreeSet;

use wordcraft_doc::para::OBJ;
use wordcraft_doc::{Comment, Document, InlineObject, PartKind, Pos, para_block};
use wordcraft_inkwitness::replay::State;

use crate::comments::{self, TeacherComment};
use crate::review::Loaded;

/// The author teacher comments carry.
pub const AUTHOR: &str = "Teacher";

/// The comment ids [`decorate`] added to a document.
pub type Added = BTreeSet<u32>;

/// Add `notes` that can be found in `st` (the state `doc` is a copy of) to `doc` as comments.
/// Returns the ids they were given.
pub fn decorate(doc: &mut Document, st: &State, notes: &[TeacherComment]) -> Added {
    let mut added = Added::new();
    // Every start and end, inserted from the last position back so earlier offsets hold. At one
    // position an end goes in before a start, so a point comment's start comes first in the text.
    let mut marks: Vec<(Pos, bool, u32)> = Vec::new();
    for n in notes {
        let Some((a, b)) = n.anchor.as_ref().and_then(|x| comments::locate(st, x)) else { continue };
        let Some(id) = wordcraft_doc::free_id(doc.comments.keys().next_back().copied(), doc.comments.keys().copied(), 0) else { break };
        let blocks = n.text.lines().take(200).map(|l| para_block(wordcraft_doc::Paragraph::with_text(l, Default::default()))).collect();
        let Ok(part) = doc.try_add_part(PartKind::Comment, blocks) else { continue };
        doc.comments.insert(
            id,
            Comment {
                author: AUTHOR.into(),
                initials: "T".into(),
                date: n.written.clone(),
                part,
                resolved: false,
                parent: None,
                ..Default::default()
            },
        );
        added.insert(id);
        marks.push((a, true, id));
        marks.push((b, false, id));
    }
    marks.sort_by(|(p, start, _), (q, other, _)| (&q.path.0, q.off, !*other).cmp(&(&p.path.0, p.off, !*start)));
    for (p, start, id) in marks {
        let obj = if start { InlineObject::CommentStart { id } } else { InlineObject::CommentEnd { id } };
        // A position that can't take the object leaves that comment without its range.
        let _ = doc.insert_object(&p, obj, &Default::default());
    }
    added
}

/// Whether the inline object for each `OBJ` in `text` is one of `added`'s comment marks.
fn ours<'a>(objects: &'a [InlineObject], added: &'a Added) -> impl FnMut(char) -> bool + 'a {
    let mut objs = objects.iter();
    move |c| c == OBJ && matches!(objs.next(), Some(InlineObject::CommentStart { id } | InlineObject::CommentEnd { id }) if added.contains(id))
}

/// Position `p` of the undecorated state, on the page `shown` (decorated with `added`).
pub fn to_shown(shown: &Document, added: &Added, p: &Pos) -> Pos {
    let Some(para) = shown.para_at(p).filter(|_| !added.is_empty()) else { return p.clone() };
    let mut is_ours = ours(&para.objects, added);
    let mut plain = 0;
    for (i, c) in para.text.char_indices() {
        if is_ours(c) {
            continue;
        }
        if plain >= p.off {
            return Pos { off: i, ..p.clone() };
        }
        plain += c.len_utf8();
    }
    Pos { off: para.text.len(), ..p.clone() }
}

/// Position `p` on the page `shown` (decorated with `added`), in the undecorated state.
pub fn from_shown(shown: &Document, added: &Added, p: &Pos) -> Pos {
    let Some(para) = shown.para_at(p).filter(|_| !added.is_empty()) else { return p.clone() };
    let mut is_ours = ours(&para.objects, added);
    let mut removed = 0;
    for (i, c) in para.text.char_indices() {
        if i >= p.off {
            break;
        }
        if is_ours(c) {
            removed += OBJ.len_utf8();
        }
    }
    Pos { off: p.off.saturating_sub(removed), ..p.clone() }
}

/// The review copy: the last recorded document with the teacher's comments as Word comments,
/// named "<file> (reviewed).docx". It carries no edit history.
pub fn review_copy(l: &Loaded, notes: &[TeacherComment]) -> Result<(String, Vec<u8>), String> {
    let mut doc = l.last.doc.clone();
    l.with_pictures(&mut doc);
    decorate(&mut doc, &l.last, notes);
    let stem = std::path::Path::new(&l.name).file_stem().and_then(|s| s.to_str()).unwrap_or("submission");
    let name = format!("{stem} (reviewed).docx");
    let bytes = wordcraft_engine::io::save_bytes(&name, &doc)?;
    Ok((name, bytes))
}
