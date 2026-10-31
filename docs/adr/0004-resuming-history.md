# ADR 0004: Keeping history across sessions (GH #9)

- Status: accepted (FAL-663). Builds on [ADR 0003](0003-sessions-and-unobserved-spans.md).
- Date: 2026-10-31
- Scope: `crates/inkwitness` (`resume.rs`, `lib.rs`), `crates/inkwitness-review/src/writer.rs`, `apps/wordcraft-web/src/web.rs`.

## Context

The writer made a recorder when the page loaded and never read the evidence in a file it opened, so a second save held only the second visit. A student who saved, came back and saved again lost the first day's history.

## Decision

When the writer opens a file (`file.open`: the picker, a drop, File > Open), the recorder looks at the DOCX it was given and does one of two things. Either way the attempt on the page before the file was opened is left behind: its events are never carried into the next attempt.

### Continue

The history is continued when all of these hold:

1. The envelope reads, and its attempt id matches the evidence's.
2. `Replay` replays every event to the recorded final state (`export.recordedStateDigest`).
3. The digest chain links from the attempt's genesis digest and every event digest matches its content (`resume::verify_chain`).
4. If the writer knows who is writing (`Recorder::set_identity`), the file's student and assignment are the same.

The recorder then takes over the file's events, checkpoints, sessions and spans as they are. The attempt id stays. Numbering carries on from the last event, the first new event's `priorEventDigest` is the last old event's digest, and new block ids never reuse a `b<n>` the history has named. A new session starts (`reason: "resume"`) with its own `sessionSpanId`. The old events keep their session, so the boundary is where `sessionSpanId` changes; the gap between the two sessions is the ADR 0003 `gap-between-sessions` span.

The identity (name, assignment, when it was entered) is the file's.

### The file was changed outside the writer

The file's document is compared with the history's recorded final state:

| File | Result |
|---|---|
| Same state digest | No event. The new session carries straight on. |
| Same body text, different digest (formatting normalised by a round trip) | One `import_document` event with a checkpoint, so replay can continue. No unobserved span, because nothing is known to have been written. |
| Different body text (edited in Word) | The same one event, and one `change-outside-writer` span (`afterSeq` = the last old event, `beforeSeq` = that event + 1). The changed text is **never** written as typing events. |

The `import_document` event is the replay's only way to start from the file as it now is. It is a marker, not a record of what changed. The reviewer shows it as already-there text, as it does for any import.

### New attempt

When the file has no history, its history is unreadable or fails any check above, or it belongs to another student, a **new attempt** starts: a new attempt id, session id and genesis. It starts from a blank page and records the file as one `import_document` event, so none of the page's earlier typing and none of the file's old events are in it. The session is `reason: "import"`, which writes the `preexisting-at-import` span ADR 0003 describes. The corrupt history is not repaired, not replayed and not copied.

### What the student is told

`Recorder::take_notices()` gives plain sentences, which the web shell puts in the status line: that a history was continued (with how many earlier sessions), that the text differs from what was last recorded, or that the file's history was not continued and why (no history, unreadable, a different student or assignment).

### Why File > New is unchanged

`file.new`, and the other commands that replace the document, still go on in the same attempt as one `import_document` event. They make a document with no earlier attempt, and nothing in them can leak a file's history.

## Consequences

- Until G1's onboarding enters an identity, `Recorder::set_identity` is not called and a valid history is continued under the identity it records. G1 must call it; the isolation test covers the entered case.
- The events of an attempt are all in one file, so growth is linear in what is written. Measured in `history_stays_inside_the_size_limits_over_many_sessions` (about 500 characters typed per session, synthetic text): the evidence part is 28 KB after one session and 254 KB after twelve, about 20 KB per session, against the 32 MB envelope limit. DOCX size 28 KB then 196 KB. Real essays type more, and each checkpoint carries a full document state, so check this again when G1 sets the checkpoint interval.
- Continuing a history means the file's evidence is trusted exactly as far as the chain check goes. A person who rewrites events and recomputes the chain will be continued. That is the same limit as `docs/DOCX_FORMAT.md` states for the file.
- The reviewer does not yet show the session boundary or the spans in its UI (ADR 0003); it reads both through `Replay::sessions()` and `Replay::unobserved_spans()`.

## Tests

- `crates/inkwitness/src/tests_resume.rs`: write, save, reopen, write, save (one chain, untouched earlier events, new session, one gap); three sessions; a Word edit (one `change-outside-writer`, no typing events, replay and chain verify); a corrupt history; a file with no history; another student's file (identity isolation: none of A's events or text in B's evidence); size growth over twelve sessions.
- `crates/inkwitness-review/src/tests.rs` `a_reopened_file_reviews_as_both_sessions`: the reviewer loads the twice-saved file as consistent and plays both sessions in order across the boundary.
