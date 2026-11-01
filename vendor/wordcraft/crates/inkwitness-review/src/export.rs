//! Teacher comments as WordCraft comments (FAL-519): drawn on the page as comment balloons, and
//! written into a separate review copy as Word comments. Teacher highlights (FAL-547) are only
//! painted on the page ([`crate::review`]); the review copy has them as Word highlights. The
//! student's file is never written.
//!
//! The comments are added to a copy of the shown document only ([`decorate`]). Each one puts two
//! objects into the text (its start and end), so offsets on the page and in the replayed state
//! differ by them; [`to_shown`] and [`from_shown`] convert between the two. At the end of the
//! history the shown document can be the teacher's edited one ([`crate::edits`]): comments and
//! highlights are placed on it through its [`Map`].

use std::collections::BTreeSet;

use wordcraft_doc::para::OBJ;
use wordcraft_doc::{Comment, Document, InlineObject, PartKind, Pos, StoryRef, para_block};
use wordcraft_inkwitness::replay::State;

use crate::comments::{self, TeacherComment};
use crate::edits::{Edits, Map, place};
use crate::review::Loaded;

/// The author teacher comments carry.
pub const AUTHOR: &str = "Teacher";

/// The comment ids [`decorate`] added to a document.
pub type Added = BTreeSet<u32>;

/// Add the comments of `notes` (not the highlights) that can be found in `st` to `doc`: a copy of
/// `st`'s document, or the teacher's edited one with `map`. Returns the ids they were given.
pub fn decorate(doc: &mut Document, st: &State, notes: &[TeacherComment], map: Option<&Map>) -> Added {
    let mut added = Added::new();
    // Every start and end, inserted from the last position back so earlier offsets hold. At one
    // position an end goes in before a start, so a point comment's start comes first in the text.
    let mut marks: Vec<(Pos, bool, u32)> = Vec::new();
    for n in notes.iter().filter(|n| n.highlight.is_none()) {
        let Some((a, b)) = n.anchor.as_ref().and_then(|x| comments::locate(st, x)) else { continue };
        let (a, b) = place(map, &a, &b);
        let Some(id) = wordcraft_doc::free_id(doc.comments.keys().next_back().copied(), doc.comments.keys().copied(), 0) else { break };
        let blocks = n.shown_text().lines().take(200).map(|l| para_block(wordcraft_doc::Paragraph::with_text(l, Default::default()))).collect();
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

/// Highlight the text of `notes`' highlights that can be found in `st` in `doc` (a copy of `st`'s
/// document, or the teacher's edited one with `map`), as Word highlights. The text doesn't
/// change, so positions still hold.
pub fn highlight(doc: &mut Document, st: &State, notes: &[TeacherComment], map: Option<&Map>) {
    for n in notes {
        let (Some(h), Some((a, b))) = (n.highlight, n.anchor.as_ref().and_then(|x| comments::locate(st, x))) else { continue };
        let (a, b) = place(map, &a, &b);
        // A range the document can't format is left as it is.
        let _ = doc.format_range(&a, &b, &|c| c.highlight = Some(h));
    }
}

/// `doc` without the comments [`decorate`] added to it (`added`): what the teacher edited on the
/// page, as their edits.
pub fn undecorate(doc: &mut Document, added: &Added) {
    if added.is_empty() {
        return;
    }
    for path in doc.para_paths(StoryRef::Body) {
        let Ok(para) = doc.para_mut(StoryRef::Body, &path) else { continue };
        let marks: Vec<usize> = {
            let mut is_ours = ours(&para.objects, added);
            para.text.char_indices().filter(|(_, c)| is_ours(*c)).map(|(i, _)| i).collect()
        };
        for at in marks.into_iter().rev() {
            let _ = para.delete(at, at + OBJ.len_utf8());
        }
    }
    for id in added {
        doc.comments.remove(id);
    }
}

/// The review copy: the last recorded document, with the teacher's edits as tracked changes in
/// their name (`edits`), their comments as Word comments and their highlights as Word
/// highlights, named "<file> (reviewed).docx". It carries no edit history.
pub fn review_copy(l: &Loaded, notes: &[TeacherComment], edits: Option<&Edits>) -> Result<(String, Vec<u8>), String> {
    let (mut doc, map) = match edits {
        Some(e) => (e.doc.clone(), Map::new(&l.last.doc, e)),
        None => (l.last.doc.clone(), None),
    };
    l.with_pictures(&mut doc);
    // The student's document decides whether Word tracks their next changes.
    doc.settings.track_changes = l.last.doc.settings.track_changes;
    highlight(&mut doc, &l.last, notes, map.as_ref());
    decorate(&mut doc, &l.last, notes, map.as_ref());
    let stem = std::path::Path::new(&l.name).file_stem().and_then(|s| s.to_str()).unwrap_or("submission");
    let name = format!("{stem} (reviewed).docx");
    let bytes = wordcraft_engine::io::save_bytes(&name, &doc)?;
    Ok((name, bytes))
}
