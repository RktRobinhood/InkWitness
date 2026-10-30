//! Teacher edits (FAL-547). With Full ribbon on, their name typed and the last recorded moment
//! shown, the teacher can edit the final document. Every change is a Word tracked change in the
//! teacher's name: the session tracks changes and its author is that name. The edits are kept in
//! a copy of the final document ([`Edits::doc`]) that only the review copy is made from; the
//! student's file and history are never changed. Stepping back through the history shows the
//! student's text at that moment, and the edits are shown again at the end.
//!
//! Teacher comments and highlights stay anchored in the student's final text. [`Map`] carries
//! positions between the student's final document and the edited one: a tracked deletion keeps
//! its text, so every character and paragraph mark of the student's is still in the edited
//! document, in the same order, with the teacher's insertions between them.

use wordcraft_doc::{Document, Pos, StoryRef};

/// The final document with the teacher's tracked changes.
#[derive(Clone)]
pub struct Edits {
    pub doc: Document,
    /// How many revisions the final document had before the teacher's: later ones are theirs.
    base: usize,
}

impl Edits {
    /// Start editing `doc`, the final document: every change from now on is tracked.
    pub fn new(mut doc: Document) -> Edits {
        doc.settings.track_changes = true;
        Edits { base: doc.revisions.len(), doc }
    }

    /// Whether `rev` is one of the teacher's revisions.
    fn theirs(&self, rev: Option<u32>) -> bool {
        rev.is_some_and(|r| r as usize >= self.base)
    }

    /// Whether the teacher has a tracked change in the document.
    pub fn changed(&self) -> bool {
        self.doc.para_paths(StoryRef::Body).iter().filter_map(|p| self.doc.para(StoryRef::Body, p)).any(|para| {
            let mark = &para.mark;
            self.theirs(mark.ins)
                || self.theirs(mark.del)
                || para.runs.iter().any(|r| {
                    self.theirs(r.props.ins) || self.theirs(r.props.del) || r.props.fmt_change.as_ref().is_some_and(|c| self.theirs(Some(c.rev)))
                })
        })
    }
}

/// The student's characters and paragraph marks in `doc`, in reading order, as positions in
/// `doc`. A character or mark that `skip` says was inserted (its `ins` revision) is left out.
fn units(doc: &Document, skip: &dyn Fn(Option<u32>) -> bool) -> Vec<Pos> {
    let mut out = Vec::new();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(para) = doc.para(StoryRef::Body, &path) else { continue };
        let mut off = 0;
        for r in &para.runs {
            let end = (off + r.len).min(para.text.len());
            if !skip(r.props.ins) {
                out.extend(para.text.get(off..end).unwrap_or_default().char_indices().map(|(i, _)| Pos::new(StoryRef::Body, path.clone(), off + i)));
            }
            off = end;
        }
        // Text no run covers (an unformatted paragraph) is the student's.
        out.extend(para.text.get(off..).unwrap_or_default().char_indices().map(|(i, _)| Pos::new(StoryRef::Body, path.clone(), off + i)));
        if !skip(para.mark.ins) {
            out.push(Pos::new(StoryRef::Body, path.clone(), para.text.len()));
        }
    }
    out
}

/// Positions of the student's final document matched up with the teacher's edited one.
pub struct Map {
    /// Each character and paragraph mark of the student's final document, in reading order.
    student: Vec<Pos>,
    /// Where each of them is in the edited document.
    edited: Vec<Pos>,
    /// Just after each of them in the edited document.
    after: Vec<Pos>,
}

impl Map {
    /// Match `student` (the final document) with `edits`. `None` when they don't match up: the
    /// edited document has a different number of the student's characters.
    pub fn new(student: &Document, edits: &Edits) -> Option<Map> {
        let student = units(student, &|_| false);
        let edited = units(&edits.doc, &|rev| edits.theirs(rev));
        if student.len() != edited.len() {
            return None;
        }
        let after = edited.iter().map(|p| after(&edits.doc, p)).collect();
        Some(Map { student, edited, after })
    }

    /// Where position `p` of the student's final document is in the edited one. The end of a
    /// range (`end`) stays right after the student's text before it, so it doesn't take in what
    /// the teacher added there.
    pub fn to_edited(&self, p: &Pos, end: bool) -> Pos {
        if p.story != StoryRef::Body {
            return p.clone();
        }
        let i = self.student.partition_point(|u| u < p);
        if end && i > 0 && self.student[i - 1].path == p.path {
            return self.after[i - 1].clone();
        }
        self.edited.get(i).or(self.edited.last()).cloned().unwrap_or_else(|| p.clone())
    }

    /// Where position `p` of the edited document is in the student's final one: in the teacher's
    /// insertion, the student's text right after it.
    pub fn to_student(&self, p: &Pos) -> Pos {
        if p.story != StoryRef::Body {
            return p.clone();
        }
        let i = self.edited.partition_point(|u| u < p);
        self.student.get(i).or(self.student.last()).cloned().unwrap_or_else(|| p.clone())
    }
}

/// Where the range `a..b` of the student's final document is on the edited one (`map`; `None`:
/// the same document). A point stays a point.
pub fn place(map: Option<&Map>, a: &Pos, b: &Pos) -> (Pos, Pos) {
    match map {
        Some(m) if a == b => {
            let p = m.to_edited(a, false);
            (p.clone(), p)
        }
        Some(m) => (m.to_edited(a, false), m.to_edited(b, true)),
        None => (a.clone(), b.clone()),
    }
}

/// The position right after the character at `p` (a paragraph mark: `p` itself).
fn after(doc: &Document, p: &Pos) -> Pos {
    let len = doc.para_at(p).and_then(|para| para.text.get(p.off..)).and_then(|t| t.chars().next()).map_or(0, char::len_utf8);
    Pos { off: p.off + len, ..p.clone() }
}
