//! Commit observation (InkWitness): a hook at [`Session::run`]'s commit boundary.
//!
//! An observer attached with [`Session::set_observer`] hears about every command that changed the
//! document, once per user action: commands that run other commands report only the outermost
//! one, a command that changed nothing reports nothing, and a failed command reports nothing
//! (it left the document as it was). Undo and Redo report the document they swapped in.
//!
//! A [`Gesture`] names what the person did (a paste) before the command it starts runs. The
//! session attaches it to the outermost run of exactly the command it was raised for, so an
//! observer can link the gesture to the operations that one command committed. Gestures never
//! travel in command params, so they never reach a recorded macro or Repeat.
//!
//! [`Session::run`]: crate::Session::run
//! [`Session::set_observer`]: crate::Session::set_observer

use serde_json::Value;
use wordcraft_doc::Document;

use crate::Selection;

/// What the person did that started a command, raised by the front end before it runs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gesture {
    /// The command the gesture is for (`edit.paste`): only an outermost run of this command
    /// takes the gesture.
    pub command: String,
    /// The clipboard formats (MIME types) the platform exposed with a paste, or `None` when the
    /// front end can't tell (eframe passes paste text only).
    pub clipboard_types: Option<Vec<String>>,
    /// Filled in by the session from the observer ([`CommitObserver::gesture_id`]) when the
    /// gesture begins; `None` without an observer.
    pub id: Option<String>,
}

impl Gesture {
    /// A paste gesture for `command` (`edit.paste`, `edit.pasteText`, `edit.pasteSpecial`).
    pub fn paste(command: &str, clipboard_types: Option<Vec<String>>) -> Self {
        Gesture { command: command.to_string(), clipboard_types, id: None }
    }
}

/// A command that pastes from the clipboard (Paste Formatting copies formatting only).
pub fn is_paste(id: &str) -> bool {
    id.starts_with("edit.paste") && id != "edit.pasteFormat"
}

/// How the document changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitKind {
    /// A command edited the document.
    Edit,
    /// Undo swapped an earlier snapshot in.
    Undo,
    /// Redo swapped a later snapshot back in.
    Redo,
    /// A command replaced the document with another (open, new, a mail merge).
    Replaced,
}

/// One committed change, as [`CommitObserver::on_commit`] sees it.
#[derive(Debug)]
pub struct Commit<'a> {
    /// The command id.
    pub command: &'a str,
    /// The command's params.
    pub params: &'a Value,
    pub kind: CommitKind,
    pub before: &'a Document,
    pub before_sel: &'a Selection,
    pub after: &'a Document,
    pub after_sel: &'a Selection,
    /// The gesture that started this command, if one was raised for it.
    pub gesture: Option<&'a Gesture>,
}

/// Hears about committed changes (see the module docs).
pub trait CommitObserver: Send {
    /// A new gesture's durable id.
    fn gesture_id(&mut self) -> String;
    /// The document changed.
    fn on_commit(&mut self, commit: &Commit<'_>);
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{Session, cmd};

    /// InkWitness review: no command changes, replaces or saves a view-only document, and none
    /// panics trying.
    #[test]
    fn a_view_only_session_refuses_every_change() {
        let params = [json!({}), json!({"text": "x", "size": 30, "style": "Heading 1", "preset": "narrow", "rows": 2, "cols": 2}), json!(null)];
        for spec in cmd::registry().all() {
            let mut s = Session::new(crate::sample::sample_document());
            s.run("caret.docStart", &json!({})).ok();
            s.run("select.word", &json!({})).ok();
            s.set_read_only(true);
            let refused = s.refuses_read_only(spec.id);
            // Commands that reach outside the session (files, printers) are only run when the
            // session refuses them before they start.
            if spec.id.starts_with("file.") && !refused {
                continue;
            }
            let (doc, id) = (s.doc.clone(), s.document_id());
            for p in &params {
                let r = s.run(spec.id, p);
                assert!(!refused || r.is_err(), "{} ran on a view-only document", spec.id);
                assert!(s.doc.same_content(&doc), "{} changed a view-only document", spec.id);
                assert_eq!(s.document_id(), id, "{} replaced a view-only document", spec.id);
            }
        }
    }

    #[test]
    fn view_only_refuses_editing_but_not_looking() {
        let mut s = Session::new(crate::sample::sample_document());
        s.set_read_only(true);
        for id in ["text.insert", "edit.paste", "edit.undo", "edit.redo", "file.new", "file.open", "file.save", "layout.margins"] {
            assert!(s.refuses_read_only(id), "{id}");
        }
        for id in ["caret.docEnd", "select.all", "edit.copy", "view.zoom", "view.readMode"] {
            assert!(!s.refuses_read_only(id), "{id}");
        }
        assert!(s.run("caret.docEnd", &json!({})).is_ok());
        assert!(s.run("select.all", &json!({})).is_ok());
        assert!(s.run("text.insert", &json!({"text": "x"})).is_err());
        s.set_read_only(false);
        assert!(s.run("text.insert", &json!({"text": "x"})).is_ok());
    }
}
