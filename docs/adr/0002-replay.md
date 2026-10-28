# ADR 0002: Replaying a recorded WordCraft session

- Status: accepted for G0 (FAL-389). Builds on `docs/adr/0001-event-anchors.md`. The op format and `v0` digests stay provisional until G1 freezes the canonical serialization.
- Date: 2026-10-28
- Scope: backlog G0-6, which is "show the actual intermediate rendered state and scroll to the exact operation range in WordCraft".

## Context

The reviewer must show the document as it was after any recorded event, in WordCraft's own page renderer, and highlight the event's range. It must not use a video, a separate text pane or a text search. WordCraft's undo history is whole-document snapshots (`struct Undo`, `engine/src/session.rs`). They are not invertible operations, and they are not in the DOCX. Replay therefore has to come from what the evidence records.

## Decision

### Where replay starts: checkpoints

- The `Recorder` writes a checkpoint when it attaches (`afterSeq: 0`). It writes another after every `import_document` event (New, Open, a mail merge), because those events carry block ids but not the new document.
- A checkpoint is the schema's `{afterSeq, canonicalDocumentStateDigest, compressedStateBase64}`. The compressed state is gzip of the JSON `{"blockIds": {story: [ids]}, "document": <WordCraft serde Document>}`. The block ids go inside the compressed state, because the schema's checkpoint object allows no other fields.
- The evidence part itself is removed from the document before it is checkpointed, so the history never nests inside itself.
- Reading a checkpoint checks two things: each story's id list has one id per paragraph, and the state matches its digest. Either failure stops the replay.

### How each event is applied

`crates/inkwitness/src/replay.rs` (`Replay::state_at`, `Replay::advance`) loads the latest checkpoint at or before the target and then applies the events in `seq` order. Event numbers must run 1, 2, 3… with no gaps.

| op | Applied as |
|---|---|
| `replace` | Find the window by its `beforeBlocks` ids, which must be consecutive in the story. Check each block's length and that the flow text at `offset` is `deleted`. Build the window's new paragraphs from `content` if present, otherwise with the plain-text rule below. Check that the new flow text and the `afterBlocks` lengths are exactly what the op says. Splice the paragraphs in and hand the ids on from `afterBlocks`. |
| `replace` with `storyBlocks` | Same checks, then the whole story is replaced (see below). |
| `setPart` | `lastSection` (page setup: size, margins, columns), `styles` or `numbering` is set to the recorded value. |
| `replaceDocument` | Load the checkpoint written for this `seq` and check its ids equal the op's `stories`. |
| `nonStory` | Nothing outside the stories is replayed for this event (comments, settings and other parts; see Known limits). |

After every event, the rebuilt state's digest must equal the event's `documentStateDigest`. At the end it must equal `export.recordedStateDigest`. Any mismatch is a `ReplayError` naming the event. The reviewer can still seek up to the event before it, and the history is labelled corrupt from there.

### The plain-text rule, and when an op carries `content`

To keep typing small (ADR 0001), a keystroke's op has no paragraph model, only its text change. Replay rebuilds it with one fixed rule (`replay::apply_text`):

- inserted text takes the formatting at the insertion point (`Paragraph::props_at`);
- a paragraph mark splits with `Paragraph::split_off`, and new middle paragraphs copy the split paragraph's properties;
- a deletion across paragraph marks merges them, keeping the first paragraph's properties.

**The recorder applies the same rule when it records.** If the rule does not rebuild exactly the paragraphs WordCraft produced, the op carries `content` even for typing. Examples are typing after Ctrl+B with nothing selected, or Enter after a heading switching to the next style. So replay is exact either way. Every command that is not typing carries `content`, as ADR 0001 already said.

### Tables and other non-paragraph structure: `storyBlocks`

An op's window is a run of the story's paragraphs in document order, table cells included. When a table comes or goes inside the window, its paragraphs are not neighbours in one block list, so replacing them cannot rebuild the table. The recorder detects this (`Op::siblings`) and adds `storyBlocks`, the story's whole top-level block list, up to 1 MiB. Replay then replaces the story. Inserting a table is pinned by a test.

### Anchors to the page

- `State::resolve(anchor, window)` maps `{blockId, startOffset, endOffset}` to WordCraft `Pos` values in the rebuilt state, through the block-id list. A range that runs past its block continues across paragraph marks (3 bytes each).
- It also recomputes the anchor's `contextHash` over the op's window. If it does not match, the reviewer labels the anchor "uncertain". It never searches for the text elsewhere.
- The page rectangles come from WordCraft's layout of that state (`DocLayout::selection_rects`). They are recomputed every frame, so margins, fonts, zoom or window width change the geometry but never which text is marked. No pixel coordinate is stored or read.

### Read-only review

- `Session::set_read_only(true)` (engine) makes `Session::run` refuse every `mutates` command. It also refuses the commands that swap, replace or write the document without that flag: Undo, Undo To, Redo, Repeat, New, Open, Close, Save, Save As, Rename, Encrypt and Recover.
- If any other command changes the document anyway, the document is put back and the command fails.
- `WordApp::execute` refuses the same commands before the web save path, which bypasses the session.
- Each frame, the review page also compares the shown document with the rebuilt state and restores it if anything changed it around the session.
- Nothing is written to the document. Restrict Editing's `review.restrict`, which stores read-only protection in the file, is not used.

## Consequences

- Evidence size grows a little. There is one checkpoint per import (a blank document's compresses to a few kilobytes). Typing carries `content` only when the rule can't rebuild it, and formatting commands carry their paragraphs as before.
- Replay checks every step, so a doctored or damaged history is reported at the first event that doesn't fit. Nothing is guessed. This is a consistency check, not a signature.

## Known limits (G0)

- **Not replayed:** comments, footnote/endnote part creation (a new story), headers and footers that did not exist at the checkpoint, document settings and media added later. Pictures inserted after the checkpoint are in `content`/`storyBlocks` as references, but their bytes are not. A new story is reported as `Unsupported`.
- **The state digest (`v0`) covers the stories only.** Page setup, styles and numbering are replayed but not covered by it. The tests compare them to the live session directly.
- **Content above size limits.** Above 256 KiB of `content`, or 1 MiB of `storyBlocks`, the op says so. Replay then falls back to the text rule, and the digest check stops it if that isn't exact.
- **Speed.** Seeking backwards replays from the checkpoint. That is fine for a G0 session; a 3,000-word essay with thousands of events needs periodic checkpoints (G1/G2).
