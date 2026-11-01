# ADR 0005: Showing a file as it stands after changes outside InkWitness (GH #10)

- Status: accepted (FAL-664). Builds on [ADR 0003](0003-sessions-and-unobserved-spans.md).
- Date: 2026-11-01
- Scope: `crates/inkwitness-review/src/outside.rs`, `review.rs`, `rail.rs`, `flags.rs`, `crates/inkwitness/src/replay.rs` (`State::with_outside_document`).

## Context

A student edited a saved file in Word and the teacher opened it in the reviewer. The history replayed, but the file's text differed from its last recorded state. The loader labelled this "External changes", kept showing the replayed end state and dropped the file's own document, so the reviewer "did not show the full file. It only showed the history."

## Decision

When the history replays and the file's body text differs from the last recorded state (`Consistency::ExternallyChanged`), the reviewer keeps the history and adds a final section, **"Changed outside InkWitness (not observed)"**, that shows the document as the file has it.

- The file opens on that section. Any step into the history (timeline, Play, Previous/Next, a flag) shows the recorded moment, and the rail's "Show the file as it stands" button comes back. It is not an event: it has no number, no time and no pin on the timeline.
- The rail lists each change, found by comparing the recorded end with the file paragraph by paragraph (longest common run of unchanged paragraphs; each run in between is one change, narrowed to the characters that differ): the text **added** (also marked on the page) and the text **removed**. Each has Promote to comment, which anchors on the added text in the file as it stands.
- Large additions are flagged like a large paste: at least 150 characters or 30 words gives `wa.outside.large-addition.v1` ("Large addition outside"), drawn as the large-paste pin and tinted like a large paste, with the sentence that when it was added and where it came from are unknown. Exact chatbot wording in added text gives `wa.text.template-marker.v1`, as for a paste (the same list, matched locally, quoted as written). At least 150 characters removed gives `wa.edit.large-delete.v1`. Nothing else is inferred.
- The **final document** for the rest of the review is the file as it stands: teacher comments, highlights and edits are anchored on it, and the review copy (docx) carries it, with no evidence part. Block ids are carried over from the recorded end where paragraphs are the same (`State::with_outside_document`), so a comment made on an earlier moment still finds its text.
- A file whose text matches its history shows no section. A corrupt history shows no section (its file text can't be compared to a state that doesn't replay). A file with no history is unchanged ("History missing").

## What this does not say

The section says InkWitness has no record of the change. It does not say when it was made, in which program, or by whom: Word's own edits, another editor and a copy of the text all look the same. The wording is a flag with alternatives, never a finding, under the same wording rules as every other flag (`flags.rs`, `outside.rs` tests).

## Limits

- Only the body text is compared (as the consistency check always did). A change to formatting, a picture, a header or a comment alone doesn't make the section appear.
- A change is shown at paragraph granularity. A very long file with a very large change (more than about 4 million paragraph pairs) is shown as one block.
- Word desktop was not available, so the "edited in Word" case is tested with a file re-saved by WordCraft's own writer without a recorder (what an editor that doesn't record does to the text). See `docs/BACKLOG.md`.
