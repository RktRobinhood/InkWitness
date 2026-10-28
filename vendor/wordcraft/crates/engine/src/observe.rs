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
