# ADR 0001: How edit events anchor to the WordCraft document model

- Status: accepted for G0 (FAL-387). The op format and digests are provisional until the DOCX envelope (FAL-388) and the replay spike (FAL-389) freeze them.
- Date: 2026-10-28
- Scope: backlog G0-3 (event observer at the commit boundary) and G0-4 (paste → `gestureId` → committed operations).

## Context

InkWitness must record every committed edit as an event of `schemas/evidence-v1.schema.json`. Each event names *where* it happened, before and after, so the reviewer can seek to it in WordCraft's own page renderer. `docs/DOCX_FORMAT.md` sets the rule:

> anchors in document model units. No display-pixel coordinates are serialized as authoritative anchors.

It also asks for "stable block/node identities plus offset and context-digest", rebound through the operations on a paragraph split or merge, not by searching for strings.

What WordCraft v0.11.0 gives us:

- One commit boundary. `Session::run` is the only entry point for the UI, CLI, MCP and control channel. Commands run other commands through it.
- A copy-on-write document. Stories are `Vec<Arc<Block>>`, and an edit copies only the paragraphs it touches.
- **No stable paragraph ids.** A position is `Pos { story, path, off }`. `path` is block indexes, which shift on every insert above, and `off` is a UTF-8 byte offset into `Paragraph::text`. `Paragraph::rev` is process-local and not serialized.
- Undo and Redo swap whole-document snapshots (`edit.undo` and `edit.redo` are `pure`, so `spec.mutates` is false for them).

## Options considered

1. **Stable block ids in the doc model.** Add an `id` to `Paragraph` and keep it through every edit primitive (split, merge, paste, table edits, the DOCX/ODT/RTF readers). This is the most direct option. But `Paragraph` is built in hundreds of places across `doc`, `docx`, `formats` and `engine`, so every one becomes an upstream patch to carry. Writing the id into OOXML (`w14:paraId`) or leaving it out has not been shown to be safe for Word round-trips. Rejected for G0.
2. **Path + offset + context digest only, re-found by searching.** Rejected: `docs/DOCX_FORMAT.md` forbids naive string search, and repeated text makes it ambiguous.
3. **Recorder-side block ids, rebound through each commit's structural diff (chosen).** InkWitness keeps an id per paragraph *beside* the document, and hands ids on at each commit by diffing the paragraphs before and after. Nothing is added to the doc model or to normal OOXML. Because the recorder sees every commit, the ids are carried through the recorded operations themselves.

## Decision

### Where events come from

- `crates/engine/src/observe.rs` (an upstream addition) defines `CommitObserver`, `Commit` and `Gesture`. `Session::run` is now a thin wrapper around the old body (`run_command`). For the **outermost** run only, it snapshots the document (a cheap `Arc` clone) and selection, and after a successful run reports a `Commit` **only if `Document::same_content` says the document changed**.
  - A no-op reports nothing. A failed command reports nothing, because `run` has already put the document back.
  - A nested command reports nothing. Its outer command's single event covers it.
  - `edit.undo` and `edit.undoTo` report `CommitKind::Undo`, and `edit.redo` reports `Redo`. This is decided by command id and an actual content change, not `spec.mutates`.
  - A command that changed `document_id` (New, Open, a mail merge, Compare) reports `Replaced`.
- `crates/inkwitness` (new, layer 5) implements the observer as `Recorder` and turns each commit into one evidence event.

### Block ids

- On attach, each paragraph of each story (body, then parts by id; table-cell paragraphs in document order) gets `b1`, `b2`, … The recorder stores them per story in document order. It holds the last document so that order stays valid.
- At each commit, per story:
  1. Strip the common prefix and suffix of paragraphs (the same `Arc` or equal content). Those paragraphs keep their ids.
  2. **Same number of paragraphs in the window:** each changed paragraph is its own op and keeps its id. This covers typing, deleting inside a paragraph and formatting.
  3. **Paragraphs came or went:** one op over the window. If one side of the window is empty, it takes in a neighbour paragraph so the paragraph mark has text on both sides. Ids are handed on by where the text went. Each old paragraph with text gives its id to the new paragraph that holds its **first surviving character**. Empty paragraphs claim next, by their place. Paragraphs left over on both sides then pair up in order, and new paragraphs left after that get fresh ids.
- What this means for structural edits (each pinned by a test in `crates/inkwitness/src/tests.rs`):
  - **Split** (`Abc|def`, Enter): `b1` stays with `Abc`, and `def` gets a new id. Enter at the very start of a paragraph keeps the id with the text and gives the new empty paragraph above a new id.
  - **Merge** (Backspace at the start of a paragraph): the first paragraph's id survives, and the second id is retired.
  - **Undo/Redo:** the swapped-in snapshot is diffed against the current document by the same rule. Undoing a split gives the text its original id back. The test asserts `["b1"]` after the undo.
  - **Document replaced:** every paragraph gets fresh ids, and an `import_document` event lists them.
  - **A change that bypassed `Session::run`** (the front end editing `session.doc` directly, or `Session::restore` after a failed dialog) is found at the next commit, because the recorder's last document no longer matches. It is recorded as its own event with `origin: "unknown"`, so the ids stay true and nothing is silently skipped.

### Anchors (model units, never pixels)

- `blockId` is the paragraph the range starts in.
- `startOffset` and `endOffset` are **UTF-8 byte offsets in that paragraph's `Paragraph::text`**, which is exactly WordCraft's `Pos::off`. Inline objects count as their U+FFFC placeholder.
- A range that runs past the paragraph (a multi-paragraph paste or deletion) continues through the following paragraphs of the story. Each paragraph mark counts as U+2029 PARAGRAPH SEPARATOR, which is 3 bytes. The schema's anchor has a single `blockId`, and this keeps it so.
- `contextHash` is SHA-256 (domain `inkwitness:anchor-context:v0`) of up to 16 characters on each side of the range, taken from the changed paragraphs.
- `beforeAnchor` is the range the change replaced, in the state before (collapsed for an insertion). `afterAnchor` is the range it left, in the state after (collapsed for a deletion). Both come from the event's first op. Every op has its own window and offsets.
- Choosing between equally small diffs: deleting `"two "` from `"one two three"` could also be read as deleting `"wo t"` one character later. The recorder slides the change to the selection before the command, or else to the caret after it. The selection only chooses between diffs of the same text change. It never decides what changed.

### Operations (provisional)

Each op is `{"op": "replace", "command", "story", "beforeBlocks": [{blockId, len}], "afterBlocks": [...], "offset", "deleted", "inserted"}`. `offset` is a byte offset in the window's flow text, which is the paragraph texts joined by U+2029. The op is reversible at the text and paragraph level: the window before is the window after with `inserted` at `offset` put back to `deleted`, split at the U+2029 marks into the listed blocks. Ops of non-typing commands also carry `content`, the changed paragraphs' full WordCraft model (runs, properties, objects), so formatting can be replayed exactly. Above 256 KiB, `content` is omitted and the op says so. Typing ops carry text only, to keep a 3,000-word essay's log small.

### Event types

| Command / change | `type` | `origin` |
|---|---|---|
| `edit.undo`, `edit.undoTo` / `edit.redo` | `undo` / `redo` | `undo` |
| Document replaced | `import_document` | `import` |
| `edit.paste*` except `edit.pasteFormat` | `paste` | `clipboard` |
| `edit.cut` | `cut` | `editor_command` |
| Only paragraph marks came or went (split, merge, new empty paragraph) | `structure_change` | by command |
| No text changed, same paragraphs | `format_change` | by command |
| Text inserted only / deleted only / both | `text_insert` / `text_delete` / `text_replace` | by command |

`keyboard` means a typing or editing-key command (`text.insert`, `text.newParagraph`, `text.backspace` and similar), not proof that a key was pressed. Everything else is `editor_command`, and a change outside `run` is `unknown`. A paragraph split carries `insertedText: " "`, a single mark and no text, so it is never a large text event (TEST_MATRIX W05).

### Paste gesture

- `WordApp::run`, the entry point for commands a person asks for (Ctrl+V, the ribbon, the context menu), raises a `Gesture` for a paste command before running it and drops it afterwards. The control channel and MCP don't go through `WordApp::run`, so agent-driven pastes carry no gesture.
- `Session::begin_gesture` asks the observer for the gesture's id, a UUID. The outermost run **of exactly that command** takes the gesture, and no other run can. The paste event gets `gestureId` and `pasteGestureId` set to that id.
- The gesture never travels in command params. Macro recordings and Repeat see the paste's own params, and a repeated paste has `gestureId: null` (tested).
- Paste is never inferred from text length or timing. A paste event is a paste command.
- `clipboardTypes` is written only when the front end knows the formats. eframe hands over the paste's text only, so the egui path leaves it out rather than guess. `sourceStatus` is always `"unknown"` and `sourceHint` is `null`.

### Digests (provisional `v0`)

- `documentStateDigest` (added to the schema as an optional event field) is SHA-256 over every story's blocks, serialized with WordCraft's serde model and domain-separated. Unchanged paragraphs reuse their cached digest, so a keystroke rehashes one paragraph.
  - It does **not** yet cover styles, numbering, comments, media or settings.
- `eventDigest` is SHA-256 (domain `inkwitness:event:v0`) of the event without `eventDigest`, as JSON with sorted keys and no whitespace. `priorEventDigest` chains the events, and the first one chains to SHA-256 of `inkwitness:genesis:v0` and the attempt id.
- This is not RFC 8785 JCS yet, and it has no cross-language test vectors. FAL-388 freezes canonicalization. None of this is a signature.

## Consequences

- Replay needs the starting ids. A checkpoint must store each story's block-id list beside the model state (FAL-388/389). From there, the ops' `beforeBlocks` and `afterBlocks` carry the ids forward deterministically.
- Upstream changes stay small:
  - `observe.rs`;
  - the `run` wrapper and three `Session` fields;
  - the paste gesture in `WordApp::run`;
  - the layer-table entry for the new crate.
  All are listed in `docs/UPSTREAM_LOCK.md`.

## Known limits (G0)

- **Clipboard formats in the browser.** The web build's text paste goes through eframe, which reads only `text/plain`, so `clipboardTypes` is absent there. Bounded proposal: have `apps/wordcraft-web/src/web.rs`'s existing DOM `paste` listener read `DataTransfer.types` and raise the gesture from the same DOM event, instead of correlating two listeners.
- **Paste Special** opens a dialog. The gesture is raised again when the dialog's OK runs the paste, so the event links to the confirmation rather than the original keystroke.
- **Tracked changes.** With Track Changes on, a deletion keeps its text as a revision, so it records as `format_change`. The student writer is not expected to run with Track Changes on.
- **Multi-command dialogs** (Page Setup) record one event per command. A failed dialog's rollback (`Session::restore`) records as an `unknown`-origin event.
- **IME composition** commits arrive as `text.insert` of the composed text. Pre-edit text is not recorded. W09 is NOT TESTED.
- The event's anchors cover the first op only, for a command that changed several separate places (Replace All). Every place is in `ops`.
